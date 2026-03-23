interface ErrorMessageProps {
  message: string
}

export default function ErrorMessage({ message }: ErrorMessageProps) {
  return (
    <div style={{
      fontSize: '14px',
      fontWeight: 400,
      color: 'var(--color-error)',
      lineHeight: 1.5,
    }}>
      {message}
    </div>
  )
}
