// 水合前渲染的 spinner（防闪烁，routing-and-guards.md §水合处理）。

export function PageSpinner() {
  return (
    <div className="flex min-h-[60vh] items-center justify-center">
      <div
        className="border-hairline border-t-primary h-8 w-8 animate-spin rounded-full border-2"
        role="status"
        aria-label="loading"
      />
    </div>
  )
}
