#!/usr/bin/env python3
"""同步 7 语言：以 en.json 为结构基准，补齐缺失键（保留已有翻译，新增键用各语言翻译）。"""
import json
from pathlib import Path

MSG_DIR = Path(__file__).resolve().parent.parent / "src" / "messages"
en = json.load(open(MSG_DIR / "en.json"))

# 各语言新键翻译（en, zh 已在 en.json/zh.json，这里给 ja/ko/de/fr/es/pt）
TRANSLATIONS = {
    "ja": {
        "common": {"retry": "再試行", "view": "表示", "back": "戻る", "menu": "メニュー"},
        "navigation": {"settings": "設定", "menu": "メニュー"},
        "status": {"healthy": "システム正常", "issues": "{count} 件の対応が必要な問題", "serviceDegraded": "サービス低下"},
        "health": {
            "healthy": "システム正常", "issues": "{count} 件の対応が必要な問題", "serviceDegraded": "サービス低下",
            "devicesReady": "{count} 台のデバイスが準備完了", "deviceNeedsAttention": "{count} 台のデバイスに注意が必要",
            "devicesNeedAttention": "{count} 台のデバイスに注意が必要", "agentOnline": "Agent オンライン",
            "agentOffline": "Agent オフライン", "agentsOffline": "{count} 個の Agent がオフライン",
            "cloudSyncOk": "クラウド同期 OK", "cloudSyncError": "クラウド同期エラー",
            "suspiciousLogin": "不審なログイン: {ip} から {count} 回失敗", "integrationError": "{count} 件の統合エラー",
            "moreIssues": "+ 他 {count} 件の問題",
        },
        "maintenance": {"banner": "計画メンテナンス中 — 一部機能が制限される場合があります"},
        "empty": {
            "devices": "デバイスがありません", "devicesDesc": "最初のデバイスを追加してリモート起動を開始",
            "addDevice": "デバイス追加", "setupGuide": "初めてですか？セットアップガイドを読む →",
            "wakes": "起動履歴がありません", "wakesDesc": "デバイスを起動するとここに表示されます",
            "wakesCta": "デバイスへ →", "integrations": "統合が未接続", "integrationsDesc": "Bemfa に接続して音声制御を有効化",
            "users": "ユーザーが見つかりません", "activity": "アクティビティがありません",
            "activityDesc": "ログインイベントと管理操作がここに表示されます", "agents": "Agent が見つかりません",
            "adminDevices": "デバイスが見つかりません", "adminIntegrations": "統合が見つかりません",
            "adminWakes": "起動履歴が見つかりません", "searchNoResults": "\"{query}\" の結果がありません",
        },
    },
    "ko": {
        "common": {"retry": "재시도", "view": "보기", "back": "뒤로", "menu": "메뉴"},
        "navigation": {"settings": "설정", "menu": "메뉴"},
        "status": {"healthy": "시스템 정상", "issues": "{count}개 문제 확인 필요", "serviceDegraded": "서비스 저하"},
        "health": {
            "healthy": "시스템 정상", "issues": "{count}개 문제 확인 필요", "serviceDegraded": "서비스 저하",
            "devicesReady": "{count}개 기기 준비됨", "deviceNeedsAttention": "{count}개 기기 주의 필요",
            "devicesNeedAttention": "{count}개 기기 주의 필요", "agentOnline": "Agent 온라인",
            "agentOffline": "Agent 오프라인", "agentsOffline": "{count}개 Agent 오프라인",
            "cloudSyncOk": "클라우드 동기화 OK", "cloudSyncError": "클라우드 동기화 오류",
            "suspiciousLogin": "의심스러운 로그인: {ip}에서 {count}회 실패", "integrationError": "{count}개 통합 오류",
            "moreIssues": "+ {count}개 추가 문제",
        },
        "maintenance": {"banner": "예정된 유지보수 중 — 일부 기능이 제한될 수 있습니다"},
        "empty": {
            "devices": "기기이 없습니다", "devicesDesc": "첫 기기를 추가하여 원격 켜기를 시작하세요",
            "addDevice": "기기 추가", "setupGuide": "처음이신가요? 설정 가이드 읽기 →",
            "wakes": "켜기 기록이 없습니다", "wakesDesc": "기기를 켜면 여기에 표시됩니다",
            "wakesCta": "기기로 →", "integrations": "연결된 통합 없음", "integrationsDesc": "Bemfa 연결하여 음성 제어 활성화",
            "users": "사용자를 찾을 수 없습니다", "activity": "활동이 없습니다",
            "activityDesc": "로그인 이벤트와 관리 작업이 여기에 표시됩니다", "agents": "Agent를 찾을 수 없습니다",
            "adminDevices": "기기를 찾을 수 없습니다", "adminIntegrations": "통합을 찾을 수 없습니다",
            "adminWakes": "켜기 기록을 찾을 수 없습니다", "searchNoResults": "\"{query}\"에 대한 결과 없음",
        },
    },
    "de": {
        "common": {"retry": "Wiederholen", "view": "Anzeigen", "back": "Zurück", "menu": "Menü"},
        "navigation": {"settings": "Einstellungen", "menu": "Menü"},
        "status": {"healthy": "Alle Systeme betriebsbereit", "issues": "{count} Probleme benötigen Aufmerksamkeit", "serviceDegraded": "Service beeinträchtigt"},
        "health": {
            "healthy": "Alle Systeme betriebsbereit", "issues": "{count} Probleme benötigen Aufmerksamkeit", "serviceDegraded": "Service beeinträchtigt",
            "devicesReady": "{count} Geräte bereit", "deviceNeedsAttention": "{count} Gerät benötigt Aufmerksamkeit",
            "devicesNeedAttention": "{count} Geräte benötigen Aufmerksamkeit", "agentOnline": "Agent online",
            "agentOffline": "Agent offline", "agentsOffline": "{count} Agents offline",
            "cloudSyncOk": "Cloud-Sync OK", "cloudSyncError": "Cloud-Sync-Fehler",
            "suspiciousLogin": "Verdächtige Anmeldung: {count} Fehlversuche von {ip}", "integrationError": "{count} Integrationsfehler",
            "moreIssues": "+ {count} weitere Probleme",
        },
        "maintenance": {"banner": "Geplante Wartung läuft — einige Funktionen können eingeschränkt sein"},
        "empty": {
            "devices": "Noch keine Geräte", "devicesDesc": "Fügen Sie Ihr erstes Gerät hinzu, um es remote zu aktivieren",
            "addDevice": "Gerät hinzufügen", "setupGuide": "Neu hier? Lesen Sie den Einrichtungsleitfaden →",
            "wakes": "Noch keine Aktivierungsverlauf", "wakesDesc": "Aktivieren Sie ein Gerät, um es hier zu sehen",
            "wakesCta": "Zu den Geräten →", "integrations": "Keine Integration verbunden", "integrationsDesc": "Verbinden Sie Bemfa für Sprachsteuerung",
            "users": "Keine Benutzer gefunden", "activity": "Noch keine Aktivität",
            "activityDesc": "Anmeldeereignisse und Admin-Aktionen werden hier angezeigt", "agents": "Keine Agents gefunden",
            "adminDevices": "Keine Geräte gefunden", "adminIntegrations": "Keine Integrationen gefunden",
            "adminWakes": "Kein Aktivierungsverlauf gefunden", "searchNoResults": "Keine Ergebnisse für „{query}\"",
        },
    },
    "fr": {
        "common": {"retry": "Réessayer", "view": "Voir", "back": "Retour", "menu": "Menu"},
        "navigation": {"settings": "Paramètres", "menu": "Menu"},
        "status": {"healthy": "Tous les systèmes opérationnels", "issues": "{count} problèmes nécessitent attention", "serviceDegraded": "Service dégradé"},
        "health": {
            "healthy": "Tous les systèmes opérationnels", "issues": "{count} problèmes nécessitent attention", "serviceDegraded": "Service dégradé",
            "devicesReady": "{count} appareils prêts", "deviceNeedsAttention": "{count} appareil nécessite attention",
            "devicesNeedAttention": "{count} appareils nécessitent attention", "agentOnline": "Agent en ligne",
            "agentOffline": "Agent hors ligne", "agentsOffline": "{count} agents hors ligne",
            "cloudSyncOk": "Sync cloud OK", "cloudSyncError": "Erreur de sync cloud",
            "suspiciousLogin": "Connexion suspecte : {count} échecs depuis {ip}", "integrationError": "{count} erreur d'intégration",
            "moreIssues": "+ {count} autres problèmes",
        },
        "maintenance": {"banner": "Maintenance planifiée en cours — certaines fonctionnalités peuvent être limitées"},
        "empty": {
            "devices": "Aucun appareil", "devicesDesc": "Ajoutez votre premier appareil pour le réveiller à distance",
            "addDevice": "Ajouter un appareil", "setupGuide": "Première fois ? Lisez le guide →",
            "wakes": "Pas encore d'historique", "wakesDesc": "Réveillez un appareil pour le voir ici",
            "wakesCta": "Vers les appareils →", "integrations": "Aucune intégration connectée", "integrationsDesc": "Connectez Bemfa pour activer le contrôle vocal",
            "users": "Aucun utilisateur trouvé", "activity": "Pas encore d'activité",
            "activityDesc": "Les événements de connexion et actions admin apparaîtront ici", "agents": "Aucun agent trouvé",
            "adminDevices": "Aucun appareil trouvé", "adminIntegrations": "Aucune intégration trouvée",
            "adminWakes": "Aucun historique trouvé", "searchNoResults": "Aucun résultat pour « {query} »",
        },
    },
    "es": {
        "common": {"retry": "Reintentar", "view": "Ver", "back": "Atrás", "menu": "Menú"},
        "navigation": {"settings": "Ajustes", "menu": "Menú"},
        "status": {"healthy": "Todos los sistemas operativos", "issues": "{count} problemas requieren atención", "serviceDegraded": "Servicio degradado"},
        "health": {
            "healthy": "Todos los sistemas operativos", "issues": "{count} problemas requieren atención", "serviceDegraded": "Servicio degradado",
            "devicesReady": "{count} dispositivos listos", "deviceNeedsAttention": "{count} dispositivo necesita atención",
            "devicesNeedAttention": "{count} dispositivos necesitan atención", "agentOnline": "Agent en línea",
            "agentOffline": "Agent desconectado", "agentsOffline": "{count} agents desconectados",
            "cloudSyncOk": "Sincronización en la nube OK", "cloudSyncError": "Error de sincronización en la nube",
            "suspiciousLogin": "Inicio de sesión sospechoso: {count} fallos desde {ip}", "integrationError": "{count} error de integración",
            "moreIssues": "+ {count} problemas más",
        },
        "maintenance": {"banner": "Mantenimiento programado en curso — algunas funciones pueden estar limitadas"},
        "empty": {
            "devices": "Sin dispositivos aún", "devicesDesc": "Añade tu primer dispositivo para activarlo remotamente",
            "addDevice": "Añadir dispositivo", "setupGuide": "¿Primera vez? Lee la guía →",
            "wakes": "Sin historial aún", "wakesDesc": "Activa un dispositivo para verlo aquí",
            "wakesCta": "Ir a dispositivos →", "integrations": "Sin integración conectada", "integrationsDesc": "Conecta Bemfa para activar el control por voz",
            "users": "Sin usuarios encontrados", "activity": "Sin actividad aún",
            "activityDesc": "Los eventos de inicio de sesión y acciones admin aparecerán aquí", "agents": "Sin agents encontrados",
            "adminDevices": "Sin dispositivos encontrados", "adminIntegrations": "Sin integraciones encontradas",
            "adminWakes": "Sin historial encontrado", "searchNoResults": "Sin resultados para \"{query}\"",
        },
    },
    "pt": {
        "common": {"retry": "Tentar novamente", "view": "Ver", "back": "Voltar", "menu": "Menu"},
        "navigation": {"settings": "Configurações", "menu": "Menu"},
        "status": {"healthy": "Todos os sistemas operacionais", "issues": "{count} problemas precisam de atenção", "serviceDegraded": "Serviço degradado"},
        "health": {
            "healthy": "Todos os sistemas operacionais", "issues": "{count} problemas precisam de atenção", "serviceDegraded": "Serviço degradado",
            "devicesReady": "{count} dispositivos prontos", "deviceNeedsAttention": "{count} dispositivo precisa de atenção",
            "devicesNeedAttention": "{count} dispositivos precisam de atenção", "agentOnline": "Agent online",
            "agentOffline": "Agent offline", "agentsOffline": "{count} agents offline",
            "cloudSyncOk": "Sincronização na nuvem OK", "cloudSyncError": "Erro de sincronização na nuvem",
            "suspiciousLogin": "Login suspeito: {count} falhas de {ip}", "integrationError": "{count} erro de integração",
            "moreIssues": "+ {count} outros problemas",
        },
        "maintenance": {"banner": "Manutenção programada em andamento — alguns recursos podem ser limitados"},
        "empty": {
            "devices": "Sem dispositivos ainda", "devicesDesc": "Adicione seu primeiro dispositivo para iniciá-lo remotamente",
            "addDevice": "Adicionar dispositivo", "setupGuide": "Primeira vez? Leia o guia →",
            "wakes": "Sem histórico ainda", "wakesDesc": "Inicie um dispositivo para vê-lo aqui",
            "wakesCta": "Ir para dispositivos →", "integrations": "Sem integração conectada", "integrationsDesc": "Conecte Bemfa para ativar controle por voz",
            "users": "Sem usuários encontrados", "activity": "Sem atividade ainda",
            "activityDesc": "Eventos de login e ações administrativas aparecerão aqui", "agents": "Sem agents encontrados",
            "adminDevices": "Sem dispositivos encontrados", "adminIntegrations": "Sem integrações encontradas",
            "adminWakes": "Sem histórico encontrado", "searchNoResults": "Sem resultados para \"{query}\"",
        },
    },
}

def deep_merge(base, additions):
    """递归合并：additions 的键加到 base（已有键保留）。"""
    for k, v in additions.items():
        if isinstance(v, dict) and isinstance(base.get(k), dict):
            deep_merge(base[k], v)
        elif k not in base:
            base[k] = v
        # 已有键保留（不覆盖已有翻译）

def sync_locale(locale):
    """同步单个 locale：以 en 结构为基准，补齐缺失的命名空间和键。"""
    path = MSG_DIR / f"{locale}.json"
    data = json.load(open(path))
    # 确保所有 en 的命名空间和键都存在（缺失的用 en 兜底）
    for ns, ns_val in en.items():
        if ns not in data:
            # 整个命名空间缺失 → 如果有翻译用翻译，否则用 en
            if locale in TRANSLATIONS and ns in TRANSLATIONS[locale]:
                data[ns] = TRANSLATIONS[locale][ns]
            elif locale == "zh":
                data[ns] = ns_val  # zh 会单独处理
            else:
                data[ns] = ns_val  # 兜底用 en
        elif isinstance(ns_val, dict):
            for key in ns_val:
                if key not in data[ns]:
                    # 单个键缺失
                    if locale in TRANSLATIONS and ns in TRANSLATIONS[locale] and key in TRANSLATIONS[locale][ns]:
                        data[ns][key] = TRANSLATIONS[locale][ns][key]
                    else:
                        data[ns][key] = ns_val[key]
    # 删除 en 不再有的命名空间（dashboard）
    for ns in list(data.keys()):
        if ns not in en:
            del data[ns]
    json.dump(data, open(path, "w"), indent=2, ensure_ascii=False)

# 先处理 zh（用脚本的新增键翻译）
ZH_NEW = {
    "common": {k: v[1] for k, v in {
        "retry": ("Retry", "重试"), "view": ("View", "查看"), "back": ("Back", "返回"), "menu": ("Menu", "菜单"),
    }.items()},
    "navigation": {"settings": "设置", "menu": "菜单"},
    "auth": {
        "creatingAccount": "创建账户中...", "sendingResetLink": "正在发送重置链接...",
        "passwordHint": "至少 8 个字符", "passwordsDontMatch": "两次密码不一致",
    },
    "status": {"healthy": "系统运行正常", "issues": "{count} 个问题需要处理", "serviceDegraded": "服务降级"},
    "health": {
        "healthy": "系统运行正常", "issues": "{count} 个问题需要处理", "serviceDegraded": "服务降级",
        "devicesReady": "{count} 台设备就绪", "deviceNeedsAttention": "{count} 台设备需要关注",
        "devicesNeedAttention": "{count} 台设备需要关注", "agentOnline": "Agent 在线",
        "agentOffline": "Agent 离线", "agentsOffline": "{count} 个 Agent 离线",
        "cloudSyncOk": "云同步正常", "cloudSyncError": "云同步错误",
        "suspiciousLogin": "可疑登录：{ip} {count} 次失败", "integrationError": "{count} 个集成错误",
        "moreIssues": "还有 {count} 个问题",
    },
    "maintenance": {"banner": "计划维护中 — 部分功能可能受限"},
    "empty": {
        "devices": "暂无设备", "devicesDesc": "添加你的第一台设备以开始远程唤醒",
        "addDevice": "添加设备", "setupGuide": "首次使用？阅读设置指南 →",
        "wakes": "暂无唤醒历史", "wakesDesc": "唤醒一台设备以在此查看",
        "wakesCta": "前往设备 →", "integrations": "未连接集成", "integrationsDesc": "连接巴法云以启用语音控制",
        "users": "未找到用户", "activity": "暂无活动",
        "activityDesc": "登录事件和管理操作将显示在此", "agents": "未找到 Agent",
        "adminDevices": "未找到设备", "adminIntegrations": "未找到集成",
        "adminWakes": "未找到唤醒历史", "searchNoResults": "没有匹配\"{query}\"的结果",
    },
}
zh_path = MSG_DIR / "zh.json"
zh = json.load(open(zh_path))
deep_merge(zh, ZH_NEW)
# 删除旧命名空间
zh.pop("dashboard", None)
# 修复 verifySuccess
zh.setdefault("auth", {})["verifySuccess"] = "邮箱已验证！正在跳转到你的设备..."
# admin 新键（zh）
for k, v in {
    "activity": "活动", "integrations": "集成", "wakes": "唤醒", "maintenance": "维护",
    "activityDesc": "登录事件和审计追踪", "devicesDesc": "跨用户设备监控",
    "agentsDesc": "跨用户 Agent 监控", "integrationsDesc": "跨用户集成监控",
    "wakesDesc": "跨用户唤醒审计历史", "maintenanceDesc": "紧急控制和维护模式",
    "filterAll": "全部", "filterStatus": "状态", "filterRole": "角色", "filterClear": "清除筛选",
    "lastLogin": "最后登录", "never": "从未", "resync": "重新同步", "disconnect": "断开连接",
    "disableDialogTitle": "禁用用户", "resetDialogTitle": "重置密码",
}.items():
    zh.setdefault("admin", {}).setdefault(k, v)
json.dump(zh, open(zh_path, "w"), indent=2, ensure_ascii=False)
print("zh.json synced")

# 处理其余 6 语言
for locale in ["ja", "ko", "de", "fr", "es", "pt"]:
    sync_locale(locale)
    print(f"{locale}.json synced")

# 验证所有 8 语言结构一致
print("\n=== Structure validation ===")
for locale in ["en", "zh", "ja", "ko", "de", "fr", "es", "pt"]:
    data = json.load(open(MSG_DIR / f"{locale}.json"))
    ns = sorted(data.keys())
    print(f"{locale}: {len(ns)} namespaces, dashboard={'YES' if 'dashboard' in ns else 'removed'}")
