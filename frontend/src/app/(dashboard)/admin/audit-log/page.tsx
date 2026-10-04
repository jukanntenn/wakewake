import { redirect } from 'next/navigation'
export default function AuditLogRedirect() {
  redirect('/admin/activity')
}
