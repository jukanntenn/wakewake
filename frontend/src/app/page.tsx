import { redirect } from 'next/navigation'

export default function Home() {
  // §3.1：首页 = Devices（/ → /devices）
  redirect('/devices')
}
