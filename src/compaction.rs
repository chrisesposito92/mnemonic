use std::sync::Arc;
use tokio_rusqlite::Connection;
use zerocopy::IntoBytes;
use crate::embedding::EmbeddingEngine;
use crate::summarization::SummarizationEngine;
use crate::error::ApiError;

// ──────────────────────────────────────────────────────────────────────────────
// Public request / response types
// ──────────────────────────────────────────────────────────────────────────────

#[derive(Debug, serde::Deserialize)]
pub struct CompactRequest {
    pub agent_id: String,
    pub threshold: Option<f32>,
    pub max_candidates: Option<u32>,
    pub dry_run: Option<bool>,
}

#[derive(Debug, serde::Serialize)]
pub struct CompactResponse {
    pub run_id: String,
    pub clusters_found: u32,
    pub memories_merged: u32,
    pub memories_created: u32,
    pub id_mapping: Vec<ClusterMapping>,
    pub truncated: bool,
}

#[derive(Debug, serde::Serialize)]
pub struct ClusterMapping {
    pub source_ids: Vec<String>,
    pub new_id: Option<String>,
}

// ──────────────────────────────────────────────────────────────────────────────
// Internal types
// ──────────────────────────────────────────────────────────────────────────────

struct CandidateMemory {
    id: String,
    content: String,
    tags: Vec<String>,
    created_at: String,
    embedding: Vec<f32>,
}

struct SimilarityPair {
    i: usize,
    j: usize,
    similarity: f32,
}

// ──────────────────────────────────────────────────────────────────────────────
// CompactionService
// ──────────────────────────────────────────────────────────────────────────────

pub struct CompactionService {
    db: Arc<Connection>,
    embedding: Arc<dyn EmbeddingEngine>,
    summarization: Option<Arc<dyn SummarizationEngine>>,
    embedding_model: String,
}

impl CompactionService {
    pub fn new(
        db: Arc<Connection>,
        embedding: Arc<dyn EmbeddingEngine>,
        summarization: Option<Arc<dyn SummarizationEngine>>,
        embedding_model: String,
    ) -> Self {
        Self { db, embedding, summarization, embedding_model }
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Pure helper functions (non-pub, synchronous, unit-testable)
// ──────────────────────────────────────────────────────────────────────────────

/// Compute cosine similarity as a dot product.
///
/// Since all embeddings are L2-normalized by EmbeddingEngine, cosine similarity
/// equals the dot product of the two vectors.
fn cosine_similarity(a: &[f32], b: &[f32]) -> f32 {
    a.iter().zip(b.iter()).map(|(x, y)| x * y).sum()
}

/// Compute all pairs (i < j) with cosine similarity >= threshold.
///
/// Returns pairs sorted by descending similarity (most similar first).
fn compute_pairs(candidates: &[CandidateMemory], threshold: f32) -> Vec<SimilarityPair> {
    let mut pairs = Vec::new();
    for i in 0..candidates.len() {
        for j in (i + 1)..candidates.len() {
            let sim = cosine_similarity(&candidates[i].embedding, &candidates[j].embedding);
            if sim >= threshold {
                pairs.push(SimilarityPair { i, j, similarity: sim });
            }
        }
    }
    pairs.sort_by(|a, b| b.similarity.partial_cmp(&a.similarity).unwrap_or(std::cmp::Ordering::Equal));
    pairs
}

/// Greedy first-match cluster assignment.
///
/// Processes pairs in descending similarity order. Once a memory joins a cluster,
/// it is not moved to another cluster. Returns only clusters with 2+ members.
fn cluster_candidates(pairs: &[SimilarityPair], num_candidates: usize) -> Vec<Vec<usize>> {
    let mut cluster_id: Vec<Option<usize>> = vec![None; num_candidates];
    let mut clusters: Vec<Vec<usize>> = Vec::new();

    for pair in pairs {
        match (cluster_id[pair.i], cluster_id[pair.j]) {
            (None, None) => {
                let id = clusters.len();
                clusters.push(vec![pair.i, pair.j]);
                cluster_id[pair.i] = Some(id);
                cluster_id[pair.j] = Some(id);
            }
            (Some(id), None) => {
                clusters[id].push(pair.j);
                cluster_id[pair.j] = Some(id);
            }
            (None, Some(id)) => {
                clusters[id].push(pair.i);
                cluster_id[pair.i] = Some(id);
            }
            (Some(_), Some(_)) => {
                // Both already assigned — first-match wins, skip
            }
        }
    }

    // Only return multi-member clusters (singletons are not merged)
    clusters.into_iter().filter(|c| c.len() >= 2).collect()
}

/// Concatenate memory content in chronological order (ascending created_at).
fn tier1_concat(memories: &[&CandidateMemory]) -> String {
    let mut sorted: Vec<&CandidateMemory> = memories.iter().copied().collect();
    sorted.sort_by(|a, b| a.created_at.cmp(&b.created_at));
    sorted.iter().map(|m| m.content.as_str()).collect::<Vec<_>>().join("\n")
}

/// Union of all tags from the given memories, deduplicated, preserving insertion order.
fn union_tags(memories: &[&CandidateMemory]) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    let mut result = Vec::new();
    for m in memories {
        for tag in &m.tags {
            if seen.insert(tag.clone()) {
                result.push(tag.clone());
            }
        }
    }
    result
}

/// Return the earliest created_at from the given memories (ISO 8601 sorts lexicographically).
fn earliest_created_at(memories: &[&CandidateMemory]) -> String {
    memories
        .iter()
        .map(|m| m.created_at.as_str())
        .min()
        .unwrap_or("")
        .to_string()
}

// ──────────────────────────────────────────────────────────────────────────────
// Async methods
// ──────────────────────────────────────────────────────────────────────────────

impl CompactionService {
    /// Fetch candidate memories for compaction along with their embeddings.
    ///
    /// Fetches max_candidates + 1 rows to detect truncation, then trims to max_candidates.
    /// Returns (candidates, truncated).
    async fn fetch_candidates(
        &self,
        agent_id: &str,
        max_candidates: u32,
    ) -> Result<(Vec<CandidateMemory>, bool), ApiError> {
        let agent_id = agent_id.to_string();
        let fetch_limit = max_candidates as i64 + 1;

        let mut candidates = self.db.call(move |c| -> Result<Vec<CandidateMemory>, rusqlite::Error> {
            let mut stmt = c.prepare(
                "SELECT m.id, m.content, m.tags, m.created_at, v.embedding
                 FROM memories m
                 JOIN vec_memories v ON v.memory_id = m.id
                 WHERE m.agent_id = ?1
                 ORDER BY m.created_at DESC
                 LIMIT ?2"
            )?;
            let rows = stmt.query_map(
                rusqlite::params![agent_id, fetch_limit],
                |row| {
                    let tags_str: String = row.get(2)?;
                    let tags: Vec<String> = serde_json::from_str(&tags_str).unwrap_or_default();
                    let bytes: Vec<u8> = row.get(4)?;
                    // SAFETY: sqlite-vec stores 384 aligned f32 values as 1536 bytes (IEEE 754 little-endian)
                    let embedding: Vec<f32> = unsafe {
                        std::slice::from_raw_parts(bytes.as_ptr() as *const f32, bytes.len() / 4).to_vec()
                    };
                    Ok(CandidateMemory {
                        id: row.get(0)?,
                        content: row.get(1)?,
                        tags,
                        created_at: row.get(3)?,
                        embedding,
                    })
                },
            )?;
            rows.collect::<Result<Vec<_>, _>>()
        }).await?;

        let truncated = candidates.len() > max_candidates as usize;
        if truncated {
            candidates.truncate(max_candidates as usize);
        }

        Ok((candidates, truncated))
    }

    /// Synthesize content for a cluster.
    ///
    /// Uses LLM (Tier 2) if a summarization engine is configured; falls back to
    /// chronological concatenation (Tier 1) on any LLM error.
    async fn synthesize_content(&self, cluster_memories: &[&CandidateMemory]) -> String {
        if let Some(engine) = &self.summarization {
            let texts: Vec<String> = cluster_memories.iter().map(|m| m.content.clone()).collect();
            match engine.summarize(&texts).await {
                Ok(summary) => return summary,
                Err(e) => {
                    tracing::warn!(error = %e, "LLM summarization failed, falling back to Tier 1");
                }
            }
        }
        tier1_concat(cluster_memories)
    }

    /// Run the full compaction pipeline.
    ///
    /// Pipeline: fetch_candidates → compute_pairs → cluster_candidates → synthesize_content
    ///           → atomic write (skipped if dry_run) → update compact_runs audit record.
    pub async fn compact(&self, req: CompactRequest) -> Result<CompactResponse, ApiError> {
        let threshold = req.threshold.unwrap_or(0.85);
        let max_candidates = req.max_candidates.unwrap_or(100);
        let dry_run = req.dry_run.unwrap_or(false);
        let agent_id = req.agent_id;

        // Create compact_runs record (status='running')
        let run_id = uuid::Uuid::now_v7().to_string();
        {
            let run_id_c = run_id.clone();
            let agent_id_c = agent_id.clone();
            self.db.call(move |c| {
                c.execute(
                    "INSERT INTO compact_runs (id, agent_id, threshold, dry_run, status)
                     VALUES (?1, ?2, ?3, ?4, 'running')",
                    rusqlite::params![run_id_c, agent_id_c, threshold, dry_run as i64],
                )
            }).await?;
        }

        // Fetch candidates
        let (candidates, truncated) = self.fetch_candidates(&agent_id, max_candidates).await?;

        // Early exit if nothing to compact
        if candidates.is_empty() {
            let run_id_c = run_id.clone();
            self.db.call(move |c| {
                c.execute(
                    "UPDATE compact_runs
                     SET status='completed', completed_at=datetime('now'),
                         clusters_found=0, memories_merged=0, memories_created=0
                     WHERE id=?1",
                    rusqlite::params![run_id_c],
                )
            }).await?;
            return Ok(CompactResponse {
                run_id,
                clusters_found: 0,
                memories_merged: 0,
                memories_created: 0,
                id_mapping: Vec::new(),
                truncated,
            });
        }

        // Cluster
        let pairs = compute_pairs(&candidates, threshold);
        let clusters = cluster_candidates(&pairs, candidates.len());

        // Process each cluster
        let mut id_mapping: Vec<ClusterMapping> = Vec::new();
        let mut write_ops: Vec<(String, Vec<String>, String, Vec<String>, String, Vec<f32>)> = Vec::new();
        // (new_id, source_ids, merged_content, tags, earliest_created_at, embedding)

        for cluster_indices in &clusters {
            let cluster_memories: Vec<&CandidateMemory> = cluster_indices
                .iter()
                .map(|&i| &candidates[i])
                .collect();

            let merged_content = self.synthesize_content(&cluster_memories).await;
            let merged_tags = union_tags(&cluster_memories);
            let earliest = earliest_created_at(&cluster_memories);
            let source_ids: Vec<String> = cluster_memories.iter().map(|m| m.id.clone()).collect();

            // Re-embed from merged content
            let merged_embedding = self.embedding.embed(&merged_content).await?;

            let new_id = uuid::Uuid::now_v7().to_string();

            if dry_run {
                id_mapping.push(ClusterMapping {
                    source_ids,
                    new_id: None,
                });
            } else {
                id_mapping.push(ClusterMapping {
                    source_ids: source_ids.clone(),
                    new_id: Some(new_id.clone()),
                });
                write_ops.push((new_id, source_ids, merged_content, merged_tags, earliest, merged_embedding));
            }
        }

        let clusters_found = clusters.len() as u32;
        let memories_merged: u32;
        let memories_created: u32;

        if dry_run {
            memories_merged = clusters.iter().map(|c| c.len() as u32).sum();
            memories_created = 0;
        } else {
            // Atomic write for all clusters
            let agent_id_c = agent_id.clone();
            let embedding_model_c = self.embedding_model.clone();
            let write_ops_c = write_ops;

            let result = self.db.call(move |c| -> Result<(u32, u32), rusqlite::Error> {
                let tx = c.transaction()?;

                let mut total_merged: u32 = 0;
                let mut total_created: u32 = 0;

                for (new_id, source_ids, merged_content, merged_tags, earliest, merged_embedding) in &write_ops_c {
                    let tags_json = serde_json::to_string(merged_tags).unwrap_or_else(|_| "[]".to_string());
                    let source_ids_json = serde_json::to_string(source_ids).unwrap_or_else(|_| "[]".to_string());
                    let embedding_bytes: Vec<u8> = merged_embedding.as_bytes().to_vec();

                    // INSERT merged memory
                    tx.execute(
                        "INSERT INTO memories (id, content, agent_id, session_id, tags, embedding_model, created_at, source_ids)
                         VALUES (?1, ?2, ?3, '', ?4, ?5, ?6, ?7)",
                        rusqlite::params![
                            new_id,
                            merged_content,
                            agent_id_c,
                            tags_json,
                            embedding_model_c,
                            earliest,
                            source_ids_json
                        ],
                    )?;

                    // INSERT vec embedding for merged memory
                    tx.execute(
                        "INSERT INTO vec_memories (memory_id, embedding) VALUES (?1, ?2)",
                        rusqlite::params![new_id, embedding_bytes],
                    )?;

                    total_created += 1;

                    // DELETE source vec entries first
                    for src_id in source_ids {
                        tx.execute(
                            "DELETE FROM vec_memories WHERE memory_id = ?1",
                            rusqlite::params![src_id],
                        )?;
                    }

                    // DELETE source memories
                    for src_id in source_ids {
                        tx.execute(
                            "DELETE FROM memories WHERE id = ?1",
                            rusqlite::params![src_id],
                        )?;
                    }

                    total_merged += source_ids.len() as u32;
                }

                tx.commit()?;
                Ok((total_merged, total_created))
            }).await;

            match result {
                Ok((merged, created)) => {
                    memories_merged = merged;
                    memories_created = created;
                }
                Err(e) => {
                    // Update compact_runs to 'failed'
                    let run_id_c = run_id.clone();
                    let _ = self.db.call(move |c| {
                        c.execute(
                            "UPDATE compact_runs SET status='failed', completed_at=datetime('now') WHERE id=?1",
                            rusqlite::params![run_id_c],
                        )
                    }).await;
                    return Err(e.into());
                }
            }
        }

        // Update compact_runs to 'completed'
        {
            let run_id_c = run_id.clone();
            let cf = clusters_found;
            let mm = memories_merged;
            let mc = memories_created;
            self.db.call(move |c| {
                c.execute(
                    "UPDATE compact_runs
                     SET status='completed', completed_at=datetime('now'),
                         clusters_found=?2, memories_merged=?3, memories_created=?4
                     WHERE id=?1",
                    rusqlite::params![run_id_c, cf, mm, mc],
                )
            }).await?;
        }

        Ok(CompactResponse {
            run_id,
            clusters_found,
            memories_merged,
            memories_created,
            id_mapping,
            truncated,
        })
    }
}

// ──────────────────────────────────────────────────────────────────────────────
// Unit tests
// ──────────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn make_candidate(id: &str, content: &str, tags: Vec<&str>, created_at: &str, embedding: Vec<f32>) -> CandidateMemory {
        CandidateMemory {
            id: id.to_string(),
            content: content.to_string(),
            tags: tags.into_iter().map(|t| t.to_string()).collect(),
            created_at: created_at.to_string(),
            embedding,
        }
    }

    // ── cosine_similarity tests ────────────────────────────────────────────────

    #[test]
    fn test_cosine_similarity_identical() {
        let a = vec![1.0_f32, 0.0, 0.0];
        let b = vec![1.0_f32, 0.0, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 1.0).abs() < 1e-6, "identical vectors should have similarity 1.0, got {}", sim);
    }

    #[test]
    fn test_cosine_similarity_orthogonal() {
        let a = vec![1.0_f32, 0.0];
        let b = vec![0.0_f32, 1.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - 0.0).abs() < 1e-6, "orthogonal vectors should have similarity 0.0, got {}", sim);
    }

    #[test]
    fn test_cosine_similarity_opposite() {
        let a = vec![1.0_f32, 0.0];
        let b = vec![-1.0_f32, 0.0];
        let sim = cosine_similarity(&a, &b);
        assert!((sim - (-1.0)).abs() < 1e-6, "opposite vectors should have similarity -1.0, got {}", sim);
    }

    // ── tier1_concat tests ─────────────────────────────────────────────────────

    #[test]
    fn test_tier1_concat_chronological_order() {
        let a = make_candidate("a", "first content", vec![], "2024-01-01", vec![1.0]);
        let b = make_candidate("b", "second content", vec![], "2024-01-02", vec![0.0]);
        // Pass in reverse order to verify sorting
        let refs: Vec<&CandidateMemory> = vec![&b, &a];
        let result = tier1_concat(&refs);
        assert_eq!(result, "first content\nsecond content", "content should be in chronological order");
    }

    // ── union_tags tests ───────────────────────────────────────────────────────

    #[test]
    fn test_union_tags_dedup() {
        let a = make_candidate("a", "", vec!["a", "b"], "2024-01-01", vec![]);
        let b = make_candidate("b", "", vec!["b", "c"], "2024-01-02", vec![]);
        let refs: Vec<&CandidateMemory> = vec![&a, &b];
        let result = union_tags(&refs);
        assert_eq!(result, vec!["a", "b", "c"], "tags should be deduplicated with insertion order preserved");
    }

    // ── cluster_candidates tests ───────────────────────────────────────────────

    #[test]
    fn test_cluster_two_similar() {
        // A and B are similar (above threshold)
        let pairs = vec![SimilarityPair { i: 0, j: 1, similarity: 0.95 }];
        let clusters = cluster_candidates(&pairs, 2);
        assert_eq!(clusters.len(), 1, "should produce exactly 1 cluster");
        assert!(clusters[0].contains(&0) && clusters[0].contains(&1), "cluster should contain both A and B");
    }

    #[test]
    fn test_cluster_below_threshold() {
        // No pairs above threshold — compute_pairs filters them out
        let pairs: Vec<SimilarityPair> = vec![];
        let clusters = cluster_candidates(&pairs, 3);
        assert_eq!(clusters.len(), 0, "no pairs means no clusters");
    }

    #[test]
    fn test_cluster_first_match() {
        // A-B=0.95 and A-C=0.90: pairs sorted desc -> A-B first, then A-C
        // A joins with B first (None, None) -> cluster 0: [A, B]
        // Then A-C: (Some(0), None) -> C joins cluster 0
        let pairs = vec![
            SimilarityPair { i: 0, j: 1, similarity: 0.95 },
            SimilarityPair { i: 0, j: 2, similarity: 0.90 },
        ];
        let clusters = cluster_candidates(&pairs, 3);
        assert_eq!(clusters.len(), 1, "all three should be in one cluster");
        assert_eq!(clusters[0].len(), 3, "cluster should have A, B, and C");
    }

    #[test]
    fn test_cluster_both_assigned_skip() {
        // A-B=0.95, C-D=0.90: cluster 0 = [A, B], cluster 1 = [C, D]
        // B-C=0.88: both already assigned -> skip
        let pairs = vec![
            SimilarityPair { i: 0, j: 1, similarity: 0.95 },
            SimilarityPair { i: 2, j: 3, similarity: 0.90 },
            SimilarityPair { i: 1, j: 2, similarity: 0.88 },
        ];
        let clusters = cluster_candidates(&pairs, 4);
        assert_eq!(clusters.len(), 2, "should produce exactly 2 clusters");
        assert_eq!(clusters[0].len(), 2, "cluster 0 should have A and B");
        assert_eq!(clusters[1].len(), 2, "cluster 1 should have C and D");
    }

    #[test]
    fn test_empty_candidates() {
        let pairs: Vec<SimilarityPair> = vec![];
        let clusters = cluster_candidates(&pairs, 0);
        assert_eq!(clusters.len(), 0, "empty input should produce 0 clusters");
    }
}
