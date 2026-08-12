// ua：User-Agent 轻量解析（§11.C.3）。
// 前端正则提取简短描述（"Chrome on macOS" / "curl" / "Unknown"），不在后端解析。

export function summarizeUA(ua: string | null): string {
  if (!ua) return 'Unknown'

  if (/curl/i.test(ua)) return 'curl'
  if (/wget/i.test(ua)) return 'wget'

  const os = /Mac OS X/.test(ua)
    ? 'macOS'
    : /Windows/.test(ua)
      ? 'Windows'
      : /Linux/.test(ua)
        ? 'Linux'
        : /Android/.test(ua)
          ? 'Android'
          : /iPhone|iPad|iPod/.test(ua)
            ? 'iOS'
            : ''

  if (/Edg\/(\d+)/.test(ua)) {
    return `Edge${os ? ' on ' + os : ''}`
  }
  if (/Chrome\/(\d+)/.test(ua) && !/Edg/.test(ua)) {
    return `Chrome${os ? ' on ' + os : ''}`
  }
  if (/Firefox\/(\d+)/.test(ua)) {
    return `Firefox${os ? ' on ' + os : ''}`
  }
  if (/Safari/.test(ua) && !/Chrome/.test(ua)) {
    return `Safari${os ? ' on ' + os : ''}`
  }

  return ua.length > 40 ? ua.slice(0, 37) + '...' : ua
}
