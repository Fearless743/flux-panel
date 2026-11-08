import { useState, useEffect, useRef } from "react";
import { Pencil, Trash2, Plus, RefreshCw, Download, ArrowUpCircle } from "lucide-react";
import toast from "react-hot-toast";

import {
  Button,
  Card,
  Field,
  Input,
  Textarea,
  Badge,
  Modal,
  PageHeader,
  Empty,
  Loading,
  Switch,
  Select,
} from "@/components/ui";

import {
  createNode,
  getNodeList,
  updateNode,
  deleteNode,
  getNodeInstallCommand,
  upgradeNode,
} from "@/api";

interface Node {
  id: number;
  name: string;
  ip: string;
  serverIp: string;
  port: string;
  tcpListenAddr?: string;
  udpListenAddr?: string;
  version?: string;
  http?: number;
  tls?: number;
  socks?: number;
  status: number;
  connectionStatus: "online" | "offline";
  copyLoading?: boolean;
  upgradeLoading?: boolean;
}

interface NodeForm {
  id: number | null;
  name: string;
  serverIp: string;
  port: string;
  tcpListenAddr: string;
  udpListenAddr: string;
  interfaceName: string;
  http: number;
  tls: number;
  socks: number;
}

// 远程升级节点后，后台静默轮询该节点：节点下载/校验/重启期间就地刷新该行
// （状态 + 版本号），全程不刷新页面；重启完成或版本变化即停止。
const UPGRADE_POLL_INTERVAL = 3000;
const UPGRADE_POLL_TIMEOUT = 120000;

// 一键自动复制安装命令
// uBlock Origin 的 prevent-clipboard-write 会拦截以 curl 开头等命令型字符串的
// navigator.clipboard.writeText 写入（表现为“ClickFix”误报），因此顺序尝试：
// 1) execCommand + 隐藏 textarea（不走 writeText 通道）
// 2) ClipboardItem write（同样不走 writeText 通道）
// 3) writeText 兜底
async function copyTextAuto(text: string): Promise<boolean> {
  // 1) execCommand + textarea
  {
    let ta: HTMLTextAreaElement | null = null;
    try {
      ta = document.createElement("textarea");
      ta.value = text;
      ta.setAttribute("readonly", "");
      ta.style.position = "fixed";
      ta.style.top = "0";
      ta.style.left = "0";
      ta.style.opacity = "0";
      ta.style.pointerEvents = "none";
      ta.style.fontSize = "16px";
      document.body.appendChild(ta);
      ta.focus();
      ta.select();
      ta.setSelectionRange(0, ta.value.length);
      if (document.execCommand("copy")) return true;
    } catch {
      // fallthrough
    } finally {
      ta?.remove();
    }
  }

  // 2) ClipboardItem
  try {
    if (typeof ClipboardItem !== "undefined" && navigator.clipboard?.write) {
      await navigator.clipboard.write([new ClipboardItem({ "text/plain": text })]);
      return true;
    }
  } catch {
    // fallthrough
  }

  // 3) writeText 兜底
  try {
    await navigator.clipboard.writeText(text);
    return true;
  } catch {
    return false;
  }
}

export default function NodePage() {
  const [nodeList, setNodeList] = useState<Node[]>([]);
  const [loading, setLoading] = useState(false);
  const [dialogVisible, setDialogVisible] = useState(false);
  const [dialogTitle, setDialogTitle] = useState("");
  const [isEdit, setIsEdit] = useState(false);
  const [submitLoading, setSubmitLoading] = useState(false);
  const [deleteModalOpen, setDeleteModalOpen] = useState(false);
  const [deleteLoading, setDeleteLoading] = useState(false);
  const [nodeToDelete, setNodeToDelete] = useState<Node | null>(null);
  const [protocolDisabled, setProtocolDisabled] = useState(false);
  const [protocolDisabledReason, setProtocolDisabledReason] = useState("");
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [form, setForm] = useState<NodeForm>({
    id: null,
    name: "",
    serverIp: "",
    port: "1000-65535",
    tcpListenAddr: "[::]",
    udpListenAddr: "[::]",
    interfaceName: "",
    http: 0,
    tls: 0,
    socks: 0,
  });
  const [errors, setErrors] = useState<Record<string, string>>({});

  const [installCommandModal, setInstallCommandModal] = useState(false);
  const [installCommand, setInstallCommand] = useState("");
  const [currentNodeName, setCurrentNodeName] = useState("");
  const installTextareaRef = useRef<HTMLTextAreaElement | null>(null);
  // 升级后的静默轮询定时器；仅就地刷新该节点所在行，不刷新页面
  const upgradePollTimerRef = useRef<ReturnType<typeof setTimeout> | null>(null);

  useEffect(() => {
    loadNodes();
  }, []);

  // 卸载时清理轮询，避免对已卸载组件 setState
  useEffect(() => {
    return () => {
      if (upgradePollTimerRef.current) {
        clearTimeout(upgradePollTimerRef.current);
        upgradePollTimerRef.current = null;
      }
    };
  }, []);

  // 打开安装命令弹窗时自动全选，方便用户直接 Ctrl+C 复制（不触发剪贴板 API 拦截）
  useEffect(() => {
    if (installCommandModal) {
      const t = setTimeout(() => installTextareaRef.current?.select(), 50);
      return () => clearTimeout(t);
    }
  }, [installCommandModal]);

  const loadNodes = async () => {
    setLoading(true);
    try {
      const res = await getNodeList();
      if (res.code === 0) {
        setNodeList(
          res.data.map((node: any) => ({
            ...node,
            connectionStatus: node.status === 1 ? "online" : "offline",
            copyLoading: false,
          }))
        );
      } else {
        toast.error(res.msg || "加载节点列表失败");
      }
    } catch {
      toast.error("网络错误，请重试");
    } finally {
      setLoading(false);
    }
  };

  const validatePort = (portStr: string): { valid: boolean; error?: string } => {
    if (!portStr || !portStr.trim()) {
      return { valid: false, error: "请输入端口" };
    }

    const trimmed = portStr.trim();
    const parts = trimmed
      .split(",")
      .map((p) => p.trim())
      .filter((p) => p);

    if (parts.length === 0) {
      return { valid: false, error: "请输入有效的端口" };
    }

    for (const part of parts) {
      if (part.includes("-")) {
        const range = part.split("-").map((p) => p.trim());
        if (range.length !== 2) {
          return { valid: false, error: `端口范围格式错误: ${part}` };
        }

        const start = parseInt(range[0]);
        const end = parseInt(range[1]);

        if (isNaN(start) || isNaN(end)) {
          return { valid: false, error: `端口必须是数字: ${part}` };
        }

        if (start < 1 || start > 65535 || end < 1 || end > 65535) {
          return { valid: false, error: `端口范围必须在 1-65535 之间: ${part}` };
        }

        if (start >= end) {
          return { valid: false, error: `起始端口必须小于结束端口: ${part}` };
        }
      } else {
        const port = parseInt(part);
        if (isNaN(port)) {
          return { valid: false, error: `端口必须是数字: ${part}` };
        }

        if (port < 1 || port > 65535) {
          return { valid: false, error: `端口必须在 1-65535 之间: ${part}` };
        }
      }
    }

    return { valid: true };
  };

  const validateForm = (): boolean => {
    const newErrors: Record<string, string> = {};

    if (!form.name.trim()) {
      newErrors.name = "请输入节点名称";
    } else if (form.name.trim().length < 2) {
      newErrors.name = "节点名称长度至少2位";
    } else if (form.name.trim().length > 50) {
      newErrors.name = "节点名称长度不能超过50位";
    }

    // 服务器 IP 由节点 agent 自动上报回填，不再手填校验

    const portValidation = validatePort(form.port);
    if (!portValidation.valid) {
      newErrors.port = portValidation.error || "端口格式错误";
    }

    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  };

  const handleAdd = () => {
    setDialogTitle("新增节点");
    setIsEdit(false);
    setDialogVisible(true);
    resetForm();
    setShowAdvanced(false);
    setProtocolDisabled(true);
    setProtocolDisabledReason("节点未在线，等待节点上线后再设置");
  };

  const handleEdit = (node: Node) => {
    setDialogTitle("编辑节点");
    setIsEdit(true);
    setForm({
      id: node.id,
      name: node.name,
      serverIp: node.serverIp || "",
      port: node.port || "1000-65535",
      tcpListenAddr: node.tcpListenAddr || "[::]",
      udpListenAddr: node.udpListenAddr || "[::]",
      interfaceName: (node as any).interfaceName || "",
      http: typeof node.http === "number" ? node.http : 1,
      tls: typeof node.tls === "number" ? node.tls : 1,
      socks: typeof node.socks === "number" ? node.socks : 1,
    });
    const offline = node.connectionStatus !== "online";
    setProtocolDisabled(offline);
    setProtocolDisabledReason(offline ? "节点未在线，等待节点上线后再设置" : "");
    setShowAdvanced(false);
    setDialogVisible(true);
  };

  const handleDelete = (node: Node) => {
    setNodeToDelete(node);
    setDeleteModalOpen(true);
  };

  const confirmDelete = async () => {
    if (!nodeToDelete) return;

    setDeleteLoading(true);
    try {
      const res = await deleteNode(nodeToDelete.id);
      if (res.code === 0) {
        toast.success("删除成功");
        setNodeList((prev) => prev.filter((n) => n.id !== nodeToDelete.id));
        setDeleteModalOpen(false);
        setNodeToDelete(null);
      } else {
        toast.error(res.msg || "删除失败");
      }
    } catch {
      toast.error("网络错误，请重试");
    } finally {
      setDeleteLoading(false);
    }
  };

  // 结束轮询并复位该行按钮状态
  const stopUpgradePolling = (nodeId: number) => {
    if (upgradePollTimerRef.current) {
      clearTimeout(upgradePollTimerRef.current);
      upgradePollTimerRef.current = null;
    }
    setNodeList((prev) => prev.map((n) => (n.id === nodeId ? { ...n, upgradeLoading: false } : n)));
  };

  // 升级指令下发后静默轮询：只就地更新该节点一行（状态/版本号），
  // 不触发整表 loading、不刷新页面；节点重启上线或版本变化即视为完成。
  const startUpgradePolling = (nodeId: number, oldVersion: string) => {
    if (upgradePollTimerRef.current) {
      clearTimeout(upgradePollTimerRef.current);
      upgradePollTimerRef.current = null;
    }
    const deadline = Date.now() + UPGRADE_POLL_TIMEOUT;
    let sawOffline = false;

    const tick = async () => {
      upgradePollTimerRef.current = null;
      let finished = false;
      try {
        const res = await getNodeList();
        if (res.code === 0 && Array.isArray(res.data)) {
          const fresh = res.data.find((item: any) => item.id === nodeId);
          if (fresh) {
            const online = fresh.status === 1;
            const version = fresh.version || "";
            if (!online) sawOffline = true;
            // 仅就地替换该行数据，其余行与本地交互状态保持不变
            setNodeList((prev) =>
              prev.map((n) =>
                n.id === nodeId
                  ? {
                      ...n,
                      ...fresh,
                      connectionStatus: online ? "online" : "offline",
                      copyLoading: n.copyLoading,
                      upgradeLoading: true,
                    }
                  : n
              )
            );
            if (online && (version !== oldVersion || sawOffline)) {
              finished = true;
              toast.success(
                version && version !== oldVersion
                  ? `节点已升级到 ${version}`
                  : `节点已重启完成，当前版本 ${version || "未知"}`
              );
            }
          }
        }
      } catch {
        // 单次轮询失败不中断，下一轮继续
      }

      if (finished) {
        stopUpgradePolling(nodeId);
        return;
      }
      if (Date.now() >= deadline) {
        toast("升级仍在进行中，请稍后手动刷新查看结果");
        stopUpgradePolling(nodeId);
        return;
      }
      upgradePollTimerRef.current = setTimeout(tick, UPGRADE_POLL_INTERVAL);
    };

    upgradePollTimerRef.current = setTimeout(tick, UPGRADE_POLL_INTERVAL);
  };

  const handleUpgrade = async (node: Node) => {
    if (node.connectionStatus !== "online") {
      toast.error("节点不在线，无法远程升级");
      return;
    }
    if (
      !window.confirm(
        `确定将节点「${node.name}」升级到最新版本？\n升级过程会短暂中断转发；失败将自动回滚。`
      )
    ) {
      return;
    }
    const oldVersion = node.version || "";
    setNodeList((prev) => prev.map((n) => (n.id === node.id ? { ...n, upgradeLoading: true } : n)));
    try {
      const res = await upgradeNode({ id: node.id, version: "latest" });
      if (res.code !== 0) {
        toast.error(res.msg || "升级失败");
        stopUpgradePolling(node.id);
        return;
      }
      toast.success("升级指令已下发，节点正在后台下载并重启");
      // 后台等待节点重启上线并刷新版本号，用户无需刷新页面
      startUpgradePolling(node.id, oldVersion);
    } catch {
      toast.error("升级请求失败，请重试");
      stopUpgradePolling(node.id);
    }
  };

  const handleCopyInstallCommand = async (node: Node) => {
    setNodeList((prev) => prev.map((n) => (n.id === node.id ? { ...n, copyLoading: true } : n)));

    try {
      const res = await getNodeInstallCommand(node.id);
      if (res.code === 0 && res.data) {
        // 一键自动复制：execCommand / ClipboardItem 绕开 uBlock 对 writeText 的拦截
        const ok = await copyTextAuto(res.data);
        if (ok) {
          toast.success("安装命令已复制到剪贴板");
        } else {
          // 仅当浏览器拒绝全部自动复制方式时才降级为手动弹窗
          setInstallCommand(res.data);
          setCurrentNodeName(node.name);
          setInstallCommandModal(true);
        }
      } else {
        toast.error(res.msg || "获取安装命令失败");
      }
    } catch {
      toast.error("获取安装命令失败");
    } finally {
      setNodeList((prev) => prev.map((n) => (n.id === node.id ? { ...n, copyLoading: false } : n)));
    }
  };

  // 弹窗降级：全选 + 键盘复制（仅自动复制被浏览器拒绝时使用）
  const handleSelectInstallCommand = () => {
    const ta = installTextareaRef.current;
    if (ta) {
      ta.focus();
      ta.select();
    }
    const isMac = /Mac|iPhone|iPad/i.test(navigator.platform || "");
    toast.success(isMac ? "已全选，请按 Cmd+C 复制" : "已全选，请按 Ctrl+C 复制");
  };

  const handleSubmit = async () => {
    if (!validateForm()) return;

    setSubmitLoading(true);

    try {
      const apiCall = isEdit ? updateNode : createNode;
      const data = { ...form };

      const res = await apiCall(data);
      if (res.code === 0) {
        toast.success(isEdit ? "更新成功" : "创建成功");
        setDialogVisible(false);

        if (isEdit) {
          setNodeList((prev) =>
            prev.map((n) =>
              n.id === form.id
                ? {
                    ...n,
                    name: form.name,
                    serverIp: form.serverIp,
                    port: form.port,
                    tcpListenAddr: form.tcpListenAddr,
                    udpListenAddr: form.udpListenAddr,
                    interfaceName: form.interfaceName,
                    http: form.http,
                    tls: form.tls,
                    socks: form.socks,
                  }
                : n
            )
          );
        } else {
          loadNodes();
        }
      } else {
        toast.error(res.msg || (isEdit ? "更新失败" : "创建失败"));
      }
    } catch {
      toast.error("网络错误，请重试");
    } finally {
      setSubmitLoading(false);
    }
  };

  const resetForm = () => {
    setForm({
      id: null,
      name: "",
      serverIp: "",
      port: "1000-65535",
      tcpListenAddr: "[::]",
      udpListenAddr: "[::]",
      interfaceName: "",
      http: 0,
      tls: 0,
      socks: 0,
    });
    setErrors({});
  };

  const isOnline = (node: Node) => node.connectionStatus === "online";
  const onlineCount = nodeList.filter((n) => n.connectionStatus === "online").length;

  const protoTiles = [
    { key: "http" as const, label: "HTTP", desc: "网页代理协议" },
    { key: "tls" as const, label: "TLS", desc: "加密传输协议" },
    { key: "socks" as const, label: "SOCKS", desc: "通用代理协议" },
  ];

  return (
    <div>
      <PageHeader
        title="节点管理"
        desc={
          nodeList.length > 0
            ? `共 ${nodeList.length} 个节点 · 在线 ${onlineCount} · 离线 ${nodeList.length - onlineCount}`
            : "管理转发节点与安装升级"
        }
        actions={
          <>
            <Button variant="outline" size="sm" onClick={loadNodes} loading={loading}>
              <RefreshCw size={15} /> 刷新
            </Button>
            <Button variant="primary" size="sm" onClick={handleAdd}>
              <Plus size={15} /> 新增
            </Button>
          </>
        }
      />

      <Card>
        {loading ? (
          <Loading text="正在加载..." />
        ) : nodeList.length === 0 ? (
          <Empty
            text="暂无节点配置，点击右上按钮开始创建"
            action={
              <Button variant="primary" size="sm" onClick={handleAdd}>
                <Plus size={15} /> 新增节点
              </Button>
            }
          />
        ) : (
          <div className="overflow-x-auto">
            <table className="flux-table min-w-[720px]" aria-label="节点列表">
              <thead>
                <tr>
                  <th className="text-left">节点</th>
                  <th className="text-left">状态</th>
                  <th className="text-right">操作</th>
                </tr>
              </thead>
              <tbody>
                {nodeList.map((node) => {
                  const online = isOnline(node);
                  return (
                    <tr key={node.id}>
                      <td>
                        <div className="min-w-0">
                          <div className="truncate text-sm font-medium text-white">{node.name}</div>
                          <div
                            className="truncate font-mono text-xs text-slate-400"
                            title={node.serverIp?.trim() || ""}
                          >
                            {node.serverIp?.trim() || "-"}
                          </div>
                          <div className="mt-0.5 text-t2xs text-slate-500">
                            版本 {node.version || "未知"}
                          </div>
                        </div>
                      </td>
                      <td>
                        <Badge tone={online ? "green" : "red"} dot>
                          {online ? "在线" : "离线"}
                        </Badge>
                      </td>
                      <td>
                        <div className="flex items-center justify-end gap-1.5">
                          <Button
                            size="sm"
                            variant="soft"
                            onClick={() => handleCopyInstallCommand(node)}
                            loading={node.copyLoading}
                            title="复制安装命令"
                          >
                            <Download size={14} /> 安装
                          </Button>
                          <Button
                            size="sm"
                            variant="outline"
                            onClick={() => handleUpgrade(node)}
                            loading={node.upgradeLoading}
                            disabled={!online}
                            title={online ? "远程升级到最新版" : "节点离线不可升级"}
                          >
                            <ArrowUpCircle size={14} /> 升级
                          </Button>
                          <Button size="sm" variant="ghost" onClick={() => handleEdit(node)} title="编辑">
                            <Pencil size={14} /> 编辑
                          </Button>
                          <Button size="sm" variant="danger" onClick={() => handleDelete(node)} title="删除">
                            <Trash2 size={14} />
                          </Button>
                        </div>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        )}
      </Card>

      <Modal
        open={dialogVisible}
        onClose={() => setDialogVisible(false)}
        title={dialogTitle || (isEdit ? "编辑节点" : "新增节点")}
        desc="服务器 IP 由节点自动上报，节点上线后自动回填，无需手动填写"
        width="max-w-2xl"
        footer={
          <>
            <Button variant="ghost" onClick={() => setDialogVisible(false)}>
              取消
            </Button>
            <Button variant="primary" onClick={handleSubmit} loading={submitLoading}>
              {submitLoading ? "提交中..." : "确定"}
            </Button>
          </>
        }
      >
        <div className="space-y-4">
          <Field label="节点名称" hint={errors.name}>
            <Input
              placeholder="请输入节点名称"
              value={form.name}
              onChange={(e) => setForm((prev) => ({ ...prev, name: e.target.value }))}
            />
          </Field>

          <Field label="服务器 IP">
            <Input
              placeholder={isEdit ? "已上报自动回填，节点上线后自动更新" : "新增后待节点上线自动获取"}
              value={form.serverIp}
              disabled
            />
          </Field>

          <Field
            label="可用端口"
            hint={errors.port || "支持单个端口(80)、多个端口(80,443)或端口范围(1000-65535)，逗号分隔"}
          >
            <Input
              placeholder="例如: 80,443,1000-65535"
              value={form.port}
              onChange={(e) => setForm((prev) => ({ ...prev, port: e.target.value }))}
              className="font-mono"
            />
          </Field>

          <div className="rounded-xl border border-white/[0.07] bg-white/[0.02]">
            <button
              type="button"
              onClick={() => setShowAdvanced((v) => !v)}
              className="flex w-full items-center justify-between px-4 py-3 text-sm font-medium text-slate-200"
            >
              高级配置
              <span className="text-xs text-slate-500">{showAdvanced ? "收起 ▲" : "展开 ▼"}</span>
            </button>
            {showAdvanced && (
              <div className="space-y-4 border-t border-white/[0.07] px-4 py-4">
                <Field
                  label="出口网卡名或 IP"
                  hint="用于多 IP 服务器指定出口，不懂的默认为空就行"
                >
                  <Input
                    placeholder="请输入出口网卡名或IP"
                    value={form.interfaceName}
                    onChange={(e) =>
                      setForm((prev) => ({ ...prev, interfaceName: e.target.value }))
                    }
                  />
                </Field>

                <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
                  <Field label="TCP 监听地址">
                    <Input
                      placeholder="如 [::]"
                      value={form.tcpListenAddr}
                      onChange={(e) =>
                        setForm((prev) => ({ ...prev, tcpListenAddr: e.target.value }))
                      }
                      className="font-mono"
                    />
                  </Field>
                  <Field label="UDP 监听地址">
                    <Input
                      placeholder="如 [::]"
                      value={form.udpListenAddr}
                      onChange={(e) =>
                        setForm((prev) => ({ ...prev, udpListenAddr: e.target.value }))
                      }
                      className="font-mono"
                    />
                  </Field>
                </div>

                <div>
                  <p className="mb-1 text-tsm font-medium text-slate-300">屏蔽协议</p>
                  <p className="mb-2 text-txs text-slate-500">
                    开启开关以屏蔽对应协议{protocolDisabled ? `（${protocolDisabledReason}）` : ""}
                  </p>
                  <div className="grid grid-cols-1 gap-3 sm:grid-cols-3">
                    {protoTiles.map((t) => (
                      <div
                        key={t.key}
                        className="rounded-xl border border-white/[0.07] bg-white/[0.03] px-3 py-3"
                      >
                        <div className="mb-1 text-sm font-medium text-slate-200">{t.label}</div>
                        <div className="mb-2 text-xs text-slate-500">{t.desc}</div>
                        <div className="flex items-center justify-between">
                          <span className="text-xs text-slate-400">
                            {form[t.key] === 1 ? "已开启" : "已关闭"}
                          </span>
                          <Switch
                            checked={form[t.key] === 1}
                            disabled={protocolDisabled}
                            onChange={(v) => setForm((prev) => ({ ...prev, [t.key]: v ? 1 : 0 }))}
                          />
                        </div>
                      </div>
                    ))}
                  </div>
                  <p className="mt-2 rounded-xl border border-red-400/20 bg-red-400/[0.06] px-3 py-2 text-xs text-red-300">
                    请不要在出口节点执行屏蔽协议，否则可能影响转发；屏蔽协议仅需在入口节点执行。
                  </p>
                  <div className="mt-3">
                    <Field label="协议快捷设置">
                      <Select
                        value=""
                        disabled={protocolDisabled}
                        onChange={(e) => {
                          const v = e.target.value;
                          if (v === "all") setForm((p) => ({ ...p, http: 1, tls: 1, socks: 1 }));
                          if (v === "none") setForm((p) => ({ ...p, http: 0, tls: 0, socks: 0 }));
                        }}
                      >
                        <option value="">批量设置…</option>
                        <option value="all">全部开启</option>
                        <option value="none">全部关闭</option>
                      </Select>
                    </Field>
                  </div>
                </div>
              </div>
            )}
          </div>
        </div>
      </Modal>

      <Modal
        open={deleteModalOpen}
        onClose={() => setDeleteModalOpen(false)}
        title="确认删除"
        desc="此操作不可恢复，请谨慎操作"
        footer={
          <>
            <Button variant="ghost" onClick={() => setDeleteModalOpen(false)}>
              取消
            </Button>
            <Button variant="danger" onClick={confirmDelete} loading={deleteLoading}>
              {deleteLoading ? "删除中..." : "确认删除"}
            </Button>
          </>
        }
      >
        <p className="text-sm text-slate-300">
          确定要删除节点 <strong className="text-white">"{nodeToDelete?.name}"</strong> 吗？
        </p>
      </Modal>

      <Modal
        open={installCommandModal}
        onClose={() => setInstallCommandModal(false)}
        title={`安装命令 - ${currentNodeName}`}
        desc="浏览器拒绝了自动复制，请手动复制以下命令到服务器执行"
        width="max-w-2xl"
        footer={
          <>
            <Button variant="ghost" onClick={() => setInstallCommandModal(false)}>
              关闭
            </Button>
            <Button variant="primary" size="sm" onClick={handleSelectInstallCommand}>
              全选命令
            </Button>
          </>
        }
      >
        <div className="relative">
          <Textarea ref={installTextareaRef} value={installCommand} readOnly rows={6} className="font-mono text-xs" onClick={(e) => (e.target as HTMLTextAreaElement).select()} />
        </div>
        <p className="mt-2 text-xs text-slate-500">提示：点击命令区域即可全选，再使用 Ctrl+C（Mac 为 Cmd+C）复制到服务器执行</p>
      </Modal>
    </div>
  );
}
