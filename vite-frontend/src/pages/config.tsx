import { useState, useEffect, useRef } from "react";
import { useNavigate } from "react-router-dom";
import { Save, Download, Upload, Settings2, TriangleAlert } from "lucide-react";
import toast from "react-hot-toast";
import {
  Button,
  Card,
  CardHead,
  Field,
  Input,
  Select,
  Switch,
  Modal,
  PageHeader,
  Loading,
} from "@/components/ui";
import { updateConfigs, exportBackup, importBackup } from "@/api";

import { isAdmin } from "@/utils/auth";
import { getCachedConfigs, clearConfigCache, updateSiteConfig } from "@/config/site";

interface ConfigItem {
  key: string;
  label: string;
  placeholder?: string;
  description?: string;
  type: "input" | "switch" | "select";
  options?: { label: string; value: string; description?: string }[];
  dependsOn?: string;
  dependsValue?: string;
}

const CONFIG_ITEMS: ConfigItem[] = [
  {
    key: "ip",
    label: "面板后端地址",
    placeholder: "请输入面板后端IP:PORT",
    description:
      "格式“ip:port”,用于对接节点时使用,ip是你安装面板服务器的公网ip,端口是安装脚本内输入的后端端口。不要套CDN,不支持https,通讯数据有加密",
    type: "input",
  },
  {
    key: "app_name",
    label: "应用名称",
    placeholder: "请输入应用名称",
    description: "在浏览器标签页和导航栏显示的应用名称",
    type: "input",
  },
  {
    key: "captcha_enabled",
    label: "启用验证码",
    description: "开启后，用户登录时需要完成验证码验证",
    type: "switch",
  },
  {
    key: "captcha_type",
    label: "验证码类型",
    description: "选择验证码的显示类型，不同类型有不同的安全级别",
    type: "select",
    dependsOn: "captcha_enabled",
    dependsValue: "true",
    options: [
      { label: "随机类型", value: "RANDOM", description: "系统随机选择验证码类型" },
      { label: "滑块验证码", value: "SLIDER", description: "拖动滑块完成拼图验证" },
      { label: "文字点选验证码", value: "WORD_IMAGE_CLICK", description: "按顺序点击指定文字" },
      { label: "旋转验证码", value: "ROTATE", description: "旋转图片到正确角度" },
      { label: "拼图验证码", value: "CONCAT", description: "拖动滑块完成图片拼接" },
    ],
  },
  {
    key: "ssl",
    label: "启用 WSS (WebSocket SSL)",
    description:
      "开启后，节点与面板之间的 WebSocket 通讯将使用 WSS 加密连接，需要在面板后端配置 SSL 证书",
    type: "switch",
  },
];

const getInitialConfigs = (): Record<string, string> => {
  if (typeof window === "undefined") return {};

  const configKeys = ["app_name", "captcha_enabled", "captcha_type", "ip", "ssl"];
  const initialConfigs: Record<string, string> = {};

  try {
    configKeys.forEach((key) => {
      const cachedValue = localStorage.getItem("vite_config_" + key);
      if (cachedValue) {
        initialConfigs[key] = cachedValue;
      }
    });
  } catch {
    /* ignore */
  }

  return initialConfigs;
};

export default function ConfigPage() {
  const navigate = useNavigate();
  const initialConfigs = getInitialConfigs();
  const [configs, setConfigs] = useState<Record<string, string>>(initialConfigs);
  const [loading, setLoading] = useState(Object.keys(initialConfigs).length === 0);
  const [saving, setSaving] = useState(false);
  const [hasChanges, setHasChanges] = useState(false);
  const [originalConfigs, setOriginalConfigs] = useState<Record<string, string>>(initialConfigs);
  const [exporting, setExporting] = useState(false);
  const [importing, setImporting] = useState(false);
  const [pendingBackup, setPendingBackup] = useState<any>(null);
  const [pendingFileName, setPendingFileName] = useState("");
  const [importOpen, setImportOpen] = useState(false);
  const fileInputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    if (!isAdmin()) {
      toast.error("权限不足，只有管理员可以访问此页面");
      navigate("/dashboard", { replace: true });
      return;
    }
  }, [navigate]);

  const loadConfigs = async (currentConfigs?: Record<string, string>) => {
    const configsToCompare = currentConfigs || configs;
    const hasInitialData = Object.keys(configsToCompare).length > 0;

    if (!hasInitialData) {
      setLoading(true);
    }

    try {
      const configData = await getCachedConfigs();

      const hasDataChanged = JSON.stringify(configData) !== JSON.stringify(configsToCompare);
      if (hasDataChanged) {
        setConfigs(configData);
        setOriginalConfigs({ ...configData });
        setHasChanges(false);
      }
    } catch {
      if (!hasInitialData) {
        toast.error("加载配置出错，请重试");
      }
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    const timer = setTimeout(() => {
      loadConfigs(initialConfigs);
    }, 100);

    return () => clearTimeout(timer);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  const handleConfigChange = (key: string, value: string) => {
    const newConfigs = { ...configs, [key]: value };

    if (key === "captcha_enabled" && value === "true") {
      if (!newConfigs.captcha_type) {
        newConfigs.captcha_type = "RANDOM";
      }
    }

    setConfigs(newConfigs);

    const hasChangesNow =
      Object.keys(newConfigs).some((k) => newConfigs[k] !== originalConfigs[k]) ||
      Object.keys(originalConfigs).some((k) => originalConfigs[k] !== newConfigs[k]);
    setHasChanges(hasChangesNow);
  };

  const handleSave = async () => {
    setSaving(true);
    try {
      const response = await updateConfigs(configs);
      if (response.code === 0) {
        toast.success("配置保存成功");

        clearConfigCache();

        const changedKeys = Object.keys(configs).filter(
          (key) => configs[key] !== originalConfigs[key]
        );

        setOriginalConfigs({ ...configs });
        setHasChanges(false);

        if (changedKeys.includes("app_name")) {
          await updateSiteConfig();
        }

        window.dispatchEvent(
          new CustomEvent("configUpdated", {
            detail: { changedKeys },
          })
        );
      } else {
        toast.error("保存配置失败: " + response.msg);
      }
    } catch {
      toast.error("保存配置出错，请重试");
    } finally {
      setSaving(false);
    }
  };

  const handleExportBackup = async () => {
    setExporting(true);
    try {
      const response = await exportBackup();
      if (response.code !== 0 || !response.data) {
        toast.error("导出失败: " + (response.msg || "未知错误"));
        return;
      }
      const blob = new Blob([JSON.stringify(response.data, null, 2)], {
        type: "application/json",
      });
      const url = URL.createObjectURL(blob);
      const now = new Date();
      const pad = (n: number) => String(n).padStart(2, "0");
      const fileName = `flux-panel-backup-${now.getFullYear()}${pad(now.getMonth() + 1)}${pad(
        now.getDate()
      )}-${pad(now.getHours())}${pad(now.getMinutes())}${pad(now.getSeconds())}.json`;
      const a = document.createElement("a");
      a.href = url;
      a.download = fileName;
      document.body.appendChild(a);
      a.click();
      document.body.removeChild(a);
      URL.revokeObjectURL(url);
      toast.success("备份文件已导出");
    } catch {
      toast.error("导出出错，请重试");
    } finally {
      setExporting(false);
    }
  };

  const handlePickImportFile = (e: React.ChangeEvent<HTMLInputElement>) => {
    const file = e.target.files?.[0];
    e.target.value = "";
    if (!file) return;
    const reader = new FileReader();
    reader.onload = () => {
      try {
        const data = JSON.parse(reader.result as string);
        if (
          !data ||
          typeof data !== "object" ||
          (!data.users && !data.nodes && !data.tunnels && !data.forwards)
        ) {
          toast.error("不是有效的备份文件");
          return;
        }
        setPendingBackup(data);
        setPendingFileName(file.name);
        setImportOpen(true);
      } catch {
        toast.error("备份文件解析失败，请选择正确的 JSON 文件");
      }
    };
    reader.onerror = () => toast.error("文件读取失败");
    reader.readAsText(file);
  };

  const handleConfirmImport = async () => {
    if (!pendingBackup) return;
    setImporting(true);
    try {
      const response = await importBackup(pendingBackup);
      if (response.code === 0) {
        toast.success("数据恢复成功，页面即将刷新");
        clearConfigCache();
        setImportOpen(false);
        setPendingBackup(null);
        setTimeout(() => window.location.reload(), 1200);
      } else {
        toast.error("导入失败: " + response.msg);
      }
    } catch {
      toast.error("导入出错，请重试");
    } finally {
      setImporting(false);
    }
  };

  const backupSummary = pendingBackup
    ? [
        `用户 ${pendingBackup.users?.length ?? 0}`,
        `节点 ${pendingBackup.nodes?.length ?? 0}`,
        `隧道 ${pendingBackup.tunnels?.length ?? 0}`,
        `转发 ${pendingBackup.forwards?.length ?? 0}`,
        `用户隧道授权 ${pendingBackup.userTunnels?.length ?? 0}`,
        `限速 ${pendingBackup.speedLimits?.length ?? 0}`,
      ].join("、")
    : "";

  const shouldShowItem = (item: ConfigItem): boolean => {
    if (!item.dependsOn || !item.dependsValue) {
      return true;
    }
    return configs[item.dependsOn] === item.dependsValue;
  };

  const renderConfigItem = (item: ConfigItem) => {
    const isChanged = hasChanges && configs[item.key] !== originalConfigs[item.key];

    switch (item.type) {
      case "input":
        return (
          <Input
            value={configs[item.key] || ""}
            onChange={(e) => handleConfigChange(item.key, e.target.value)}
            placeholder={item.placeholder}
            className={isChanged ? "!border-amber-400/50" : undefined}
          />
        );

      case "switch":
        return (
          <div className="flex items-center gap-3">
            <Switch
              checked={configs[item.key] === "true"}
              onChange={(checked) => handleConfigChange(item.key, checked ? "true" : "false")}
            />
            <span className="text-sm text-slate-300">
              {configs[item.key] === "true" ? "已启用" : "已禁用"}
              {isChanged && <span className="ml-2 text-xs text-amber-300">● 未保存</span>}
            </span>
          </div>
        );

      case "select":
        return (
          <Select
            value={configs[item.key] || ""}
            onChange={(e) => handleConfigChange(item.key, e.target.value)}
            className={isChanged ? "[&>select]:!border-amber-400/50" : undefined}
          >
            <option value="">请选择验证码类型</option>
            {item.options?.map((option) => (
              <option key={option.value} value={option.value} title={option.description}>
                {option.label}
              </option>
            )) || null}
          </Select>
        );

      default:
        return null;
    }
  };

  if (loading) {
    return (
      <div>
        <PageHeader title="网站配置" desc="管理网站的基本信息和显示设置" />
        <Card>
          <Loading text="加载配置中..." />
        </Card>
      </div>
    );
  }

  return (
    <div className="mx-auto max-w-4xl">
      <PageHeader
        title="网站配置"
        desc="管理网站的基本信息和显示设置"
        actions={
          <Button variant="primary" size="sm" onClick={handleSave} loading={saving} disabled={!hasChanges}>
            <Save size={15} /> {saving ? "保存中..." : "保存配置"}
          </Button>
        }
      />

      <Card>
        <CardHead
          title="基本设置"
          desc="配置网站的基本信息，这些设置会影响网站的显示效果"
          right={<Settings2 size={18} className="text-slate-500" />}
        />
        <div className="space-y-6 px-5 pb-5">
          {CONFIG_ITEMS.map((item) => {
            if (!shouldShowItem(item)) return null;
            return (
              <div key={item.key} className="border-t border-white/[0.06] pt-5 first:border-0 first:pt-1">
                <Field label={item.label} hint={item.description}>
                  <div className="mt-1.5">{renderConfigItem(item)}</div>
                </Field>
              </div>
            );
          })}
        </div>
      </Card>

      {hasChanges && (
        <div className="mt-4 flex items-center gap-2 rounded-2xl border border-amber-400/25 bg-amber-400/[0.07] px-4 py-3 text-sm text-amber-200">
          <TriangleAlert size={16} className="shrink-0" />
          检测到配置变更，请记得保存您的修改
        </div>
      )}

      <Card className="mt-4">
        <CardHead
          title="数据备份"
          desc="一键导出全部数据（用户、节点、隧道、转发、授权、限速、网站配置、流量统计），或从备份文件全量恢复"
        />
        <div className="px-5 pb-5">
          <div className="flex flex-col gap-3 sm:flex-row">
            <Button variant="primary" size="sm" onClick={handleExportBackup} loading={exporting}>
              <Download size={15} /> {exporting ? "导出中..." : "导出全部数据"}
            </Button>
            <Button variant="danger" size="sm" onClick={() => fileInputRef.current?.click()}>
              <Upload size={15} /> 从备份文件恢复
            </Button>
            <input
              ref={fileInputRef}
              type="file"
              accept=".json,application/json"
              className="hidden"
              onChange={handlePickImportFile}
            />
          </div>
          <p className="mt-4 text-xs leading-relaxed text-slate-500">
            注意：恢复为全量覆盖操作，会清空当前所有数据并按备份文件原样写入（含管理员账号与密码），操作不可撤销，请先导出当前数据再恢复。
          </p>
        </div>
      </Card>

      <Modal
        open={importOpen}
        onClose={() => setImportOpen(false)}
        title="确认恢复数据？"
        desc="该操作不可撤销，请确认已备份当前数据"
        footer={
          <>
            <Button variant="ghost" onClick={() => setImportOpen(false)} disabled={importing}>
              取消
            </Button>
            <Button variant="danger" onClick={handleConfirmImport} loading={importing}>
              {importing ? "恢复中..." : "确认覆盖恢复"}
            </Button>
          </>
        }
      >
        <p className="text-sm text-slate-300">
          即将从文件 <span className="font-mono font-medium text-white">{pendingFileName}</span>{" "}
          全量恢复，当前所有数据将被覆盖：
        </p>
        <p className="mt-2 rounded-xl border border-white/10 bg-white/[0.04] p-3 text-sm text-slate-300">
          {backupSummary}
        </p>
      </Modal>
    </div>
  );
}
