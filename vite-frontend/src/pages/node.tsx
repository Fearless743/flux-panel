import { useState, useEffect } from "react";
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

  useEffect(() => {
    loadNodes();
  }, []);

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

  const validateIp = (ip: string): boolean => {
    if (!ip || !ip.trim()) return false;

    const trimmedIp = ip.trim();

    const ipv4Regex =
      /^(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)$/;

    const ipv6Regex =
      /^(([0-9a-fA-F]{1,4}:){7,7}[0-9a-fA-F]{1,4}|([0-9a-fA-F]{1,4}:){1,7}:|([0-9a-fA-F]{1,4}:){1,6}:[0-9a-fA-F]{1,4}|([0-9a-fA-F]{1,4}:){1,5}(:[0-9a-fA-F]{1,4}){1,2}|([0-9a-fA-F]{1,4}:){1,4}(:[0-9a-fA-F]{1,4}){1,3}|([0-9a-fA-F]{1,4}:){1,3}(:[0-9a-fA-F]{1,4}){1,4}|([0-9a-fA-F]{1,4}:){1,2}(:[0-9a-fA-F]{1,4}){1,5}|[0-9a-fA-F]{1,4}:((:[0-9a-fA-F]{1,4}){1,6})|:((:[0-9a-fA-F]{1,4}){1,7}|:)|fe80:(:[0-9a-fA-F]{0,4}){0,4}%[0-9a-zA-Z]{1,}|::(ffff(:0{1,4}){0,1}:){0,1}((25[0-5]|(2[0-4]|1{0,1}[0-9]){0,1}[0-9])\.){3,3}(25[0-5]|(2[0-4]|1{0,1}[0-9]){0,1}[0-9])|([0-9a-fA-F]{1,4}:){1,4}:((25[0-5]|(2[0-4]|1{0,1}[0-9]){0,1}[0-9])\.){3,3}(25[0-5]|(2[0-4]|1{0,1}[0-9]){0,1}[0-9]))$/;

    if (ipv4Regex.test(trimmedIp) || ipv6Regex.test(trimmedIp) || trimmedIp === "localhost") {
      return true;
    }

    if (/^\d+$/.test(trimmedIp)) return false;

    const domainRegex =
      /^[a-zA-Z0-9]([a-zA-Z0-9\-]{0,61}[a-zA-Z0-9])?(\.[a-zA-Z0-9]([a-zA-Z0-9\-]{0,61}[a-zA-Z0-9])?)+$/;
    const singleLabelDomain = /^[a-zA-Z][a-zA-Z0-9\-]{0,62}$/;

    return domainRegex.test(trimmedIp) || singleLabelDomain.test(trimmedIp);
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

    if (!form.serverIp.trim()) {
      newErrors.serverIp = "请输入服务器IP地址";
    } else if (!validateIp(form.serverIp.trim())) {
      newErrors.serverIp = "请输入有效的IPv4、IPv6地址或域名";
    }

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
    setNodeList((prev) => prev.map((n) => (n.id === node.id ? { ...n, upgradeLoading: true } : n)));
    try {
      const res = await upgradeNode({ id: node.id, version: "latest" });
      if (res.code === 0) {
        toast.success(typeof res.data === "string" ? res.data : "升级指令已下发，节点即将重启");
        setTimeout(() => {
          loadNodes();
        }, 8000);
      } else {
        toast.error(res.msg || "升级失败");
      }
    } catch {
      toast.error("升级请求失败，请重试");
    } finally {
      setNodeList((prev) =>
        prev.map((n) => (n.id === node.id ? { ...n, upgradeLoading: false } : n))
      );
    }
  };

  const handleCopyInstallCommand = async (node: Node) => {
    setNodeList((prev) => prev.map((n) => (n.id === node.id ? { ...n, copyLoading: true } : n)));

    try {
      const res = await getNodeInstallCommand(node.id);
      if (res.code === 0 && res.data) {
        try {
          await navigator.clipboard.writeText(res.data);
          toast.success("安装命令已复制到剪贴板");
        } catch {
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

  const handleManualCopy = async () => {
    try {
      await navigator.clipboard.writeText(installCommand);
      toast.success("安装命令已复制到剪贴板");
      setInstallCommandModal(false);
    } catch {
      toast.error("复制失败，请手动选择文本复制");
    }
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
        desc="服务器 IP 是你要添加的服务器地址，不是面板的 IP"
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

          <Field label="服务器 IP" hint={errors.serverIp}>
            <Input
              placeholder="如: 192.168.1.100 或 example.com"
              value={form.serverIp}
              onChange={(e) => setForm((prev) => ({ ...prev, serverIp: e.target.value }))}
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
        desc="自动复制失败，请手动复制以下命令到服务器执行"
        width="max-w-2xl"
        footer={
          <>
            <Button variant="ghost" onClick={() => setInstallCommandModal(false)}>
              关闭
            </Button>
            <Button variant="primary" size="sm" onClick={handleManualCopy}>
              复制
            </Button>
          </>
        }
      >
        <div className="relative">
          <Textarea value={installCommand} readOnly rows={6} className="font-mono text-xs" />
        </div>
        <p className="mt-2 text-xs text-slate-500">提示：如果复制按钮失效，请手动选择上方文本复制</p>
      </Modal>
    </div>
  );
}
