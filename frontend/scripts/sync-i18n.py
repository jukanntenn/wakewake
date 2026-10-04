#!/usr/bin/env python3
"""Sync i18n: 补全 en.json 新键 + 删除旧键 + 生成 7 语言翻译。"""

import json
import copy
from pathlib import Path

MSG_DIR = Path(__file__).resolve().parent.parent / "src" / "messages"

# ---- 新增键（en + zh）----
NEW_COMMON = {
    "retry": ("Retry", "重试"),
    "view": ("View", "查看"),
    "back": ("Back", "返回"),
    "menu": ("Menu", "菜单"),
}
NEW_NAV = {
    "settings": ("Settings", "设置"),
    "menu": ("Menu", "菜单"),
}
NEW_AUTH = {
    "creatingAccount": ("Creating account...", "创建账户中..."),
    "sendingResetLink": ("Sending reset link...", "正在发送重置链接..."),
    "passwordHint": ("8+ characters", "至少 8 个字符"),
    "passwordsDontMatch": ("Passwords don't match", "两次密码不一致"),
}
NEW_AUTH_ERROR = {
    "MAINTENANCE_REGISTRATION_CLOSED": (
        "Registration is temporarily disabled.",
        "注册暂时关闭。",
    ),
    "MAINTENANCE_READONLY": (
        "The system is in read-only mode. Try again later.",
        "系统处于只读模式，请稍后重试。",
    ),
    "MAINTENANCE_FULL": (
        "The system is under maintenance. Try again later.",
        "系统维护中，请稍后重试。",
    ),
    "SYNCING": (
        "This item's state was changed by another action. Please refresh.",
        "该项状态已被其他操作更改，请刷新。",
    ),
    "USER_DISABLED": (
        "Your account has been disabled. Contact the administrator.",
        "你的账户已被禁用。请联系管理员。",
    ),
}
NEW_DEVICE = {
    "wakeTimeout": (
        "Wake timed out — agent did not respond in time",
        "唤醒超时 — agent 未及时响应",
    ),
    "wakeFailed": ("Wake failed: {message}", "唤醒失败：{message}"),
    "deleteDialogTitle": ("Delete device", "删除设备"),
    "deleteDialogDesc": (
        'Permanently remove "{name}"? Cloud topic will be cleaned up automatically. This cannot be undone.',
        '永久删除"{name}"？云端 topic 将被自动清理。此操作不可撤销。',
    ),
    "quotaExceeded": (
        "Device limit reached. You can have at most {max} devices. Remove an existing device first.",
        "已达设备上限（最多 {max} 台）。请先删除旧设备。",
    ),
    "addDevice": ("+ Add device", "+ 添加设备"),
    "wakeUnavailable": ("Wake unavailable", "无法唤醒"),
}
NEW_AGENT = {
    "confirmRotate": ("Click again to confirm", "再次点击确认"),
    "publicKeyNotAvailable": (
        "Not available until agent connects",
        "Agent 连接前不可用",
    ),
}
NEW_INTEGRATIONS = {
    "deleteDialogTitle": ("Remove integration", "移除集成"),
    "deleteDialogDesc": (
        "Disconnect Bemfa? Device topics are retained on the cloud (reusable), but voice wake will be unavailable.",
        "断开巴法云？设备 topic 会保留在云端（可复用），但语音唤醒将不可用。",
    ),
    "toggleSuccess": ("{action} succeeded", "{action}成功"),
    "toggleFailed": ("{action} failed", "{action}失败"),
}
NEW_SETTINGS = {
    "passwordsDontMatch": ("Passwords don't match", "两次密码不一致"),
}

# 完整新命名空间
NEW_NS = {
    "status": {
        "healthy": ("All systems operational", "系统运行正常"),
        "issues": ("{count} issues need attention", "{count} 个问题需要处理"),
        "serviceDegraded": ("Service degraded", "服务降级"),
    },
    "health": {
        "healthy": ("All systems operational", "系统运行正常"),
        "issues": ("{count} issues need attention", "{count} 个问题需要处理"),
        "serviceDegraded": ("Service degraded", "服务降级"),
        "devicesReady": ("{count} devices ready", "{count} 台设备就绪"),
        "deviceNeedsAttention": (
            "{count} device needs attention",
            "{count} 台设备需要关注",
        ),
        "devicesNeedAttention": (
            "{count} devices need attention",
            "{count} 台设备需要关注",
        ),
        "agentOnline": ("Agent online", "Agent 在线"),
        "agentOffline": ("Agent offline", "Agent 离线"),
        "agentsOffline": ("{count} agents offline", "{count} 个 Agent 离线"),
        "cloudSyncOk": ("Cloud sync OK", "云同步正常"),
        "cloudSyncError": ("Cloud sync error", "云同步错误"),
        "suspiciousLogin": (
            "Suspicious login: {count} fails from {ip}",
            "可疑登录：{ip} {count} 次失败",
        ),
        "integrationError": ("{count} integration error", "{count} 个集成错误"),
        "moreIssues": ("+ {count} more issues", "还有 {count} 个问题"),
    },
    "maintenance": {
        "banner": (
            "Scheduled maintenance in progress — some features may be limited",
            "计划维护中 — 部分功能可能受限",
        ),
    },
    "empty": {
        "devices": ("No devices yet", "暂无设备"),
        "devicesDesc": (
            "Add your first device to start waking it remotely",
            "添加你的第一台设备以开始远程唤醒",
        ),
        "addDevice": ("Add device", "添加设备"),
        "setupGuide": (
            "First time? Read the setup guide →",
            "首次使用？阅读设置指南 →",
        ),
        "wakes": ("No wake history yet", "暂无唤醒历史"),
        "wakesDesc": ("Wake a device to see it here", "唤醒一台设备以在此查看"),
        "wakesCta": ("Go to devices →", "前往设备 →"),
        "integrations": ("No integration connected", "未连接集成"),
        "integrationsDesc": (
            "Connect Bemfa to enable voice control",
            "连接巴法云以启用语音控制",
        ),
        "users": ("No users found", "未找到用户"),
        "activity": ("No activity yet", "暂无活动"),
        "activityDesc": (
            "Login events and admin actions will appear here",
            "登录事件和管理操作将显示在此",
        ),
        "agents": ("No agents found", "未找到 Agent"),
        "adminDevices": ("No devices found", "未找到设备"),
        "adminIntegrations": ("No integrations found", "未找到集成"),
        "adminWakes": ("No wake history found", "未找到唤醒历史"),
        "searchNoResults": ('No results for "{query}"', '没有匹配"{query}"的结果'),
    },
}

# admin 新增键（en, zh）——扁平结构
NEW_ADMIN_FLAT = {
    "activity": ("Activity", "活动"),
    "integrations": ("Integrations", "集成"),
    "wakes": ("Wakes", "唤醒"),
    "maintenance": ("Maintenance", "维护"),
    "activityDesc": ("Login events and audit trail", "登录事件和审计追踪"),
    "devicesDesc": ("Cross-user device monitoring", "跨用户设备监控"),
    "agentsDesc": ("Cross-user agent monitoring", "跨用户 Agent 监控"),
    "integrationsDesc": ("Cross-user integration monitoring", "跨用户集成监控"),
    "wakesDesc": ("Cross-user wake audit history", "跨用户唤醒审计历史"),
    "maintenanceDesc": (
        "Emergency controls and maintenance mode",
        "紧急控制和维护模式",
    ),
    "backToAdmin": ("Admin", "管理"),
    "searchUsers": ("Type the beginning of an email...", "输入邮箱开头..."),
    "searchDevices": ("Device name or user email...", "设备名或用户邮箱..."),
    "searchActivity": ("Email, IP, or action...", "邮箱、IP 或操作..."),
    "filterAll": ("All", "全部"),
    "filterStatus": ("Status", "状态"),
    "filterRole": ("Role", "角色"),
    "filterCloud": ("Cloud", "云同步"),
    "filterType": ("Type", "类型"),
    "filterResult": ("Result", "结果"),
    "filterDate": ("Date", "日期"),
    "filterClear": ("Clear filters", "清除筛选"),
    "lastLogin": ("Last login", "最后登录"),
    "never": ("Never", "从未"),
    "thisIsYou": ("This is you — actions disabled", "这是你 — 操作已禁用"),
    "disableDialogTitle": ("Disable user", "禁用用户"),
    "disableDialogDesc": (
        'Disable "{email}"? They will be immediately logged out and their agent disconnected.',
        '禁用"{email}"？该用户将被立即登出，其 Agent 连接断开。',
    ),
    "disableDialogReason": ("Reason (optional):", "原因（可选）："),
    "disableDialogReasonPlaceholder": (
        "e.g. Abuse — bulk registration",
        "如：滥用 — 批量注册",
    ),
    "resetDialogTitle": ("Reset password", "重置密码"),
    "resetDialogDesc": (
        'Set a new password for "{email}". Their old password will stop working and they\'ll be logged out everywhere.',
        '为"{email}"设置新密码。旧密码将失效，所有设备将被登出。',
    ),
    "resetDialogPasswordLabel": (
        "New password (min 8 characters):",
        "新密码（至少 8 个字符）：",
    ),
    "resync": ("Resync", "重新同步"),
    "resyncSuccess": ("Resync triggered", "已触发重新同步"),
    "disconnect": ("Disconnect", "断开连接"),
    "disconnectSuccess": ("Agent disconnected", "Agent 已断开"),
    "disconnectFailedHint": ("Agent is not online", "Agent 不在线"),
    "email": ("User", "用户"),
    "actions": ("Actions", "操作"),
    "statusActive": ("Active", "启用"),
}
NEW_ADMIN_PAGINATION = {
    "showing": ("Showing {from}–{to} of {total}", "显示 {total} 条中的 {from}–{to}"),
    "prev": ("Previous", "上一页"),
    "next": ("Next", "下一页"),
    "perPage": ("{count} / page", "{count} / 页"),
}
NEW_ADMIN_MAINTENANCE = {
    "title": ("Maintenance", "维护"),
    "statusNormal": ("Normal — no maintenance active", "正常 — 无维护进行中"),
    "statusActive": ("Maintenance active", "维护进行中"),
    "enable": ("Enable maintenance mode", "启用维护模式"),
    "disable": ("Disable maintenance", "禁用维护"),
    "affectsAll": (
        "This affects all users. Choose a severity:",
        "这会影响所有用户。选择严重程度：",
    ),
    "mode_registration": ("Pause registration", "暂停注册"),
    "mode_registrationDesc": (
        "New signups blocked. Existing users unaffected.",
        "阻止新注册。现有用户不受影响。",
    ),
    "mode_readonly": ("Read-only", "只读模式"),
    "mode_readonlyDesc": (
        "All write operations blocked. Users can still view.",
        "阻止所有写操作。用户仍可查看。",
    ),
    "mode_full": ("Full lockdown", "完全锁定"),
    "mode_fullDesc": (
        "Only admins can sign in. All others turned away.",
        "仅管理员可登录。其他用户被拒绝。",
    ),
    "messageLabel": ("Message to users (optional):", "给用户的消息（可选）："),
    "messagePlaceholder": ("Scheduled maintenance in progress", "计划维护进行中"),
}

# 要删除的键
OBSOLETE = {
    "dashboard": None,  # 整个命名空间删除
    "device.deleteConfirm": None,
    "agent.rotateConfirm": None,
    "integrations.deleteConfirm": None,
}


def update_en():
    en = json.load(open(MSG_DIR / "en.json"))
    en.setdefault("common", {}).update({k: v[0] for k, v in NEW_COMMON.items()})
    en.setdefault("navigation", {}).update({k: v[0] for k, v in NEW_NAV.items()})
    en.setdefault("auth", {}).update({k: v[0] for k, v in NEW_AUTH.items()})
    en.setdefault("auth", {}).setdefault("error", {}).update(
        {k: v[0] for k, v in NEW_AUTH_ERROR.items()}
    )
    en.setdefault("device", {}).update({k: v[0] for k, v in NEW_DEVICE.items()})
    en.setdefault("agent", {}).update({k: v[0] for k, v in NEW_AGENT.items()})
    en.setdefault("integrations", {}).update(
        {k: v[0] for k, v in NEW_INTEGRATIONS.items()}
    )
    en.setdefault("settings", {}).update({k: v[0] for k, v in NEW_SETTINGS.items()})
    en.setdefault("admin", {}).update({k: v[0] for k, v in NEW_ADMIN_FLAT.items()})
    en["admin"]["pagination"] = {k: v[0] for k, v in NEW_ADMIN_PAGINATION.items()}
    en["admin"]["maintenance"] = {k: v[0] for k, v in NEW_ADMIN_MAINTENANCE.items()}
    for ns, keys in NEW_NS.items():
        en[ns] = {k: v[0] for k, v in keys.items()}
    # 修复 verifySuccess（移除 dashboard 引用）
    en["auth"]["verifySuccess"] = "Email verified! Taking you to your devices..."
    # 删除旧键
    en.pop("dashboard", None)
    for key in ["deleteConfirm"]:
        en.get("device", {}).pop(key, None)
        en.get("agent", {}).pop(key, None)
        en.get("integrations", {}).pop(key, None)
    json.dump(en, open(MSG_DIR / "en.json", "w"), indent=2, ensure_ascii=False)
    print(
        f"en.json: {sum(len(v) if isinstance(v, dict) else 1 for v in en.values())} total keys"
    )


if __name__ == "__main__":
    update_en()
    print("Done. Run translations for other locales manually or via separate script.")
