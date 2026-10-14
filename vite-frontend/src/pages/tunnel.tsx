import { useState, useEffect } from "react";
import { Pencil, Trash2, Plus, Stethoscope, X, Route } from "lucide-react";
import { clsx } from "clsx";
import toast from "react-hot-toast";

import {
  Button,
  Card,
  Field,
  Input,
  Textarea,
  Select,
  Badge,
  Modal,
  PageHeader,
  Empty,
  Loading,
  Switch,
} from "@/components/ui";

import {
  createTunnel,
  getTunnelList,
  updateTunnel,
  deleteTunnel,
  getNodeList,
  diagnoseTunnel,
} from "@/api";

interface ChainTunnel {
  nodeId: number;
  protocol?: string;
  strategy?: string;
  chainType?: number;
  inx?: number;
  exitNodeIds?: number[];
}

interface Tunnel {
  id: number;
  name: string;
  type: number;
  inNodeId: ChainTunnel[];
  outNodeId?: ChainTunnel[];
  chainNodes?: ChainTunnel[][];
  inIp: string;
  outIp?: string;
  protocol?: string;
  flow: number;
  trafficRatio: number;
  status: number;
  createdTime: string;
}

interface Node {
  id: number;
  name: string;
  status: number;
}

interface TunnelForm {
  id?: number;
  name: string;
  type: number;
  inNodeId: ChainTunnel[];
  outNodeId?: ChainTunnel[];
  chainNodes?: ChainTunnel[][];
  flow: number;
  trafficRatio: number;
  inIp: string;
  status: number;
}

interface DiagnosisResult {
  tunnelName: string;
  tunnelType: string;
  timestamp: number;
  results: Array<{
    success: boolean;
    description: string;
    nodeName: string;
    nodeId: string;
    targetIp: string;
    targetPort?: number;
    message?: string;
    averageTime?: number;
    packetLoss?: number;
    fromChainType?: number;
    fromInx?: number;
    toChainType?: number;
    toInx?: number;
  }>;
}

const PROTOCOLS = ["tls", "wss", "tcp", "mtls", "mwss", "mtcp"];
const PROTOCOL_LABEL: Record<string, string> = {
  tls: "TLS",
  wss: "WSS",
  tcp: "TCP",
  mtls: "MTLS",
  mwss: "MWSS",
  mtcp: "MTCP",
};
const STRATEGIES = [
  { value: "fifo", label: "主备" },
  { value: "round", label: "轮询" },
  { value: "rand", label: "随机" },
];

function parseExitIds(entry: ChainTunnel): number[] {
  const raw: any = entry.exitNodeIds;
  if (!raw) return [];
  if (typeof raw === "string") {
    try {
      const v = JSON.parse(raw);
      return Array.isArray(v) ? v : [];
    } catch {
      return [];
    }
  }
  return Array.isArray(raw) ? raw : [];
}

function NodeChipMulti({
  nodes,
  selected,
  isDisabled,
  disableReason,
  onToggle,
}: {
  nodes: Node[];
  selected: number[];
  isDisabled: (id: number) => { disabled: boolean; reason?: string };
  disableReason?: (node: Node) => string | undefined;
  onToggle: (id: number) => void;
}) {
  return (
    <div className="flex flex-wrap gap-2">
      {nodes.map((node) => {
        const st = isDisabled(node.id);
        const active = selected.includes(node.id);
        const online = node.status === 1;
        const extra = disableReason?.(node);
        return (
          <button
            key={node.id}
            type="button"
            disabled={st.disabled}
            title={st.disabled ? st.reason || extra || "不可选" : node.name}
            onClick={() => onToggle(node.id)}
            className={clsx(
              "inline-flex items-center gap-2 rounded-xl border px-3 py-1.5 text-xs font-medium transition-all",
              active
                ? "border-blue-400/50 bg-blue-400/10 text-blue-200"
                : "border-white/10 bg-white/[0.03] text-slate-300 hover:border-white/20 hover:bg-white/[0.06]",
              st.disabled && "cursor-not-allowed opacity-40 hover:bg-white/[0.03]"
            )}
          >
            <span
              className={clsx(
                "flex h-3.5 w-3.5 items-center justify-center rounded border text-[10px]",
                active
                  ? "border-blue-300 bg-blue-400 text-slate-950"
                  : "border-slate-500 text-transparent"
              )}
            >
              ✓
            </span>
            <span className="max-w-[10rem] truncate">{node.name}</span>
            <span
              className={clsx(
                "h-1.5 w-1.5 rounded-full",
                online ? "bg-emerald-400" : "bg-slate-500"
              )}
            />
            {!online && <span className="text-slate-500">离线</span>}
            {st.reason && st.disabled && st.reason !== "离线" && (
              <span className="text-amber-300/80">{st.reason}</span>
            )}
            {extra && !st.disabled && <span className="text-amber-300/80">{extra}</span>}
          </button>
        );
      })}
      {nodes.length === 0 && <span className="text-xs text-slate-500">暂无节点</span>}
    </div>
  );
}

export default function TunnelPage() {
  const [loading, setLoading] = useState(true);
  const [tunnels, setTunnels] = useState<Tunnel[]>([]);
  const [nodes, setNodes] = useState<Node[]>([]);

  const [modalOpen, setModalOpen] = useState(false);
  const [deleteModalOpen, setDeleteModalOpen] = useState(false);
  const [diagnosisModalOpen, setDiagnosisModalOpen] = useState(false);
  const [isEdit, setIsEdit] = useState(false);
  const [submitLoading, setSubmitLoading] = useState(false);
  const [deleteLoading, setDeleteLoading] = useState(false);
  const [diagnosisLoading, setDiagnosisLoading] = useState(false);
  const [tunnelToDelete, setTunnelToDelete] = useState<Tunnel | null>(null);
  const [currentDiagnosisTunnel, setCurrentDiagnosisTunnel] = useState<Tunnel | null>(null);
  const [diagnosisResult, setDiagnosisResult] = useState<DiagnosisResult | null>(null);

  const [form, setForm] = useState<TunnelForm>({
    name: "",
    type: 1,
    inNodeId: [],
    outNodeId: [],
    chainNodes: [],
    flow: 1,
    trafficRatio: 1.0,
    inIp: "",
    status: 1,
  });

  const [errors, setErrors] = useState<{ [key: string]: string }>({});

  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    setLoading(true);
    try {
      const [tunnelsRes, nodesRes] = await Promise.all([getTunnelList(), getNodeList()]);

      if (tunnelsRes.code === 0) {
        setTunnels(tunnelsRes.data || []);
      } else {
        toast.error(tunnelsRes.msg || "获取隧道列表失败");
      }

      if (nodesRes.code === 0) {
        setNodes(nodesRes.data || []);
      } else {
        console.warn("获取节点列表失败:", nodesRes.msg);
      }
    } catch (error) {
      console.error("加载数据失败:", error);
      toast.error("加载数据失败");
    } finally {
      setLoading(false);
    }
  };

  const validateForm = (): boolean => {
    const newErrors: { [key: string]: string } = {};

    if (!form.name.trim()) {
      newErrors.name = "请输入隧道名称";
    } else if (form.name.length < 2 || form.name.length > 50) {
      newErrors.name = "隧道名称长度应在2-50个字符之间";
    }

    if (!form.inNodeId || form.inNodeId.length === 0) {
      newErrors.inNodeId = "请至少选择一个入口节点";
    } else {
      const offlineNodes = form.inNodeId.filter((item) => {
        const node = nodes.find((n) => n.id === item.nodeId);
        return node && node.status !== 1;
      });
      if (offlineNodes.length > 0) {
        newErrors.inNodeId = "存在离线入口节点，请先取消勾选移除后再保存";
      }
    }

    if (form.trafficRatio < 0.0 || form.trafficRatio > 100.0) {
      newErrors.trafficRatio = "流量倍率必须在0.0-100.0之间";
    }

    if (form.type === 2) {
      if (!form.outNodeId || form.outNodeId.filter((n) => n.nodeId !== -1).length === 0) {
        newErrors.outNodeId = "请至少选择一个出口节点";
      } else {
        const offlineNodes = (form.outNodeId || []).filter((item) => {
          if (item.nodeId === -1) return false;
          const node = nodes.find((n) => n.id === item.nodeId);
          return node && node.status !== 1;
        });
        if (offlineNodes.length > 0) {
          newErrors.outNodeId = "存在离线出口节点，请先取消勾选移除后再保存";
        }

        const inNodeIds = form.inNodeId.map((item) => item.nodeId);
        const outNodeIds = (form.outNodeId || [])
          .filter((n) => n.nodeId !== -1)
          .map((item) => item.nodeId);
        const overlap = inNodeIds.filter((id) => outNodeIds.includes(id));
        if (overlap.length > 0) {
          newErrors.outNodeId = "隧道转发模式下，入口和出口不能有相同节点";
        }
      }

      const offlineChainNodes = (form.chainNodes || [])
        .flat()
        .filter((item) => item.nodeId !== -1)
        .filter((item) => {
          const node = nodes.find((n) => n.id === item.nodeId);
          return node && node.status !== 1;
        });
      if (offlineChainNodes.length > 0) {
        newErrors.chainNodes = "存在离线转发链节点，请先取消勾选移除后再保存";
      }
    }

    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  };

  const handleAdd = () => {
    setIsEdit(false);
    setForm({
      name: "",
      type: 1,
      inNodeId: [],
      outNodeId: [],
      chainNodes: [],
      flow: 1,
      trafficRatio: 1.0,
      inIp: "",
      status: 1,
    });
    setErrors({});
    setModalOpen(true);
  };

  const handleEdit = (tunnel: Tunnel) => {
    setIsEdit(true);

    setForm({
      id: tunnel.id,
      name: tunnel.name,
      type: tunnel.type,
      inNodeId: tunnel.inNodeId || [],
      outNodeId: tunnel.outNodeId || [],
      chainNodes: tunnel.chainNodes || [],
      flow: tunnel.flow,
      trafficRatio: tunnel.trafficRatio,
      inIp: tunnel.inIp ? tunnel.inIp.split(",").map((ip) => ip.trim()).join("\n") : "",
      status: tunnel.status,
    });
    setErrors({});
    setModalOpen(true);
  };

  const handleDelete = (tunnel: Tunnel) => {
    setTunnelToDelete(tunnel);
    setDeleteModalOpen(true);
  };

  const confirmDelete = async () => {
    if (!tunnelToDelete) return;

    setDeleteLoading(true);
    try {
      const response = await deleteTunnel(tunnelToDelete.id);
      if (response.code === 0) {
        toast.success("删除成功");
        setDeleteModalOpen(false);
        setTunnelToDelete(null);
        loadData();
      } else {
        toast.error(response.msg || "删除失败");
      }
    } catch (error) {
      console.error("删除失败:", error);
      toast.error("删除失败");
    } finally {
      setDeleteLoading(false);
    }
  };

  const handleTypeChange = (type: number) => {
    setForm((prev) => ({
      ...prev,
      type,
      outNodeId: type === 1 ? [] : prev.outNodeId,
      chainNodes: type === 1 ? [] : prev.chainNodes,
    }));
  };

  const removeChainNode = (groupIndex: number) => {
    setForm((prev) => ({
      ...prev,
      chainNodes: (prev.chainNodes || []).filter((_, index) => index !== groupIndex),
    }));
  };

  const toggleChainNode = (groupIndex: number, nodeId: number) => {
    setForm((prev) => {
      const chainNodes = [...(prev.chainNodes || [])];
      const group = chainNodes[groupIndex] || [];
      if (group.some((n) => n.nodeId === nodeId)) {
        chainNodes[groupIndex] = group.filter((node) => node.nodeId !== nodeId);
      } else {
        const strategy = group.length > 0 ? group[0].strategy : "round";
        const protocol = group.length > 0 ? group[0].protocol : "tls";
        chainNodes[groupIndex] = [...group, { nodeId, chainType: 2, protocol, strategy }];
      }
      return { ...prev, chainNodes };
    });
  };

  const updateChainProtocol = (groupIndex: number, protocol: string) => {
    setForm((prev) => {
      const chainNodes = [...(prev.chainNodes || [])];
      chainNodes[groupIndex] = (chainNodes[groupIndex] || []).map((node) => ({
        ...node,
        protocol,
      }));
      return { ...prev, chainNodes };
    });
  };

  const updateChainStrategy = (groupIndex: number, strategy: string) => {
    setForm((prev) => {
      const chainNodes = [...(prev.chainNodes || [])];
      chainNodes[groupIndex] = (chainNodes[groupIndex] || []).map((node) => ({
        ...node,
        strategy,
      }));
      return { ...prev, chainNodes };
    });
  };

  const getSelectedChainNodeIds = (): number[] => {
    return (form.chainNodes || []).flatMap((group) => group.map((node) => node.nodeId));
  };

  const getChainGroups = (): ChainTunnel[][] => {
    return form.chainNodes || [];
  };

  const nodeById = (id: number) => nodes.find((n) => n.id === id);
  const inIds = form.inNodeId.map((ct) => ct.nodeId);
  const outIds = (form.outNodeId || []).filter((n) => n.nodeId !== -1).map((ct) => ct.nodeId);
  const chainIds = getSelectedChainNodeIds();

  const toggleInNode = (id: number) => {
    setForm((prev) => {
      if (prev.inNodeId.some((ct) => ct.nodeId === id)) {
        return { ...prev, inNodeId: prev.inNodeId.filter((ct) => ct.nodeId !== id) };
      }
      return { ...prev, inNodeId: [...prev.inNodeId, { nodeId: id, chainType: 1 }] };
    });
  };

  const toggleOutNode = (id: number) => {
    setForm((prev) => {
      const current = (prev.outNodeId || []).filter((ct) => ct.nodeId !== -1);
      if (current.some((ct) => ct.nodeId === id)) {
        return { ...prev, outNodeId: current.filter((ct) => ct.nodeId !== id) };
      }
      let protocol = "tls";
      let strategy = "round";
      if (current.length > 0) {
        protocol = current[0].protocol || "tls";
        strategy = current[0].strategy || "round";
      }
      return { ...prev, outNodeId: [...current, { nodeId: id, chainType: 3, protocol, strategy }] };
    });
  };

  const setOutProtoStrategy = (kind: "protocol" | "strategy", value: string) => {
    setForm((prev) => {
      const current = prev.outNodeId || [];
      if (current.length === 0) {
        return {
          ...prev,
          outNodeId: [
            {
              nodeId: -1,
              chainType: 3,
              protocol: kind === "protocol" ? value : "tls",
              strategy: kind === "strategy" ? value : "round",
            },
          ],
        };
      }
      return {
        ...prev,
        outNodeId: current.map((ct) => ({ ...ct, [kind]: value })),
      };
    });
  };

  const handleSubmit = async () => {
    if (!validateForm()) return;

    setSubmitLoading(true);
    try {
      const cleanedChainNodes = (form.chainNodes || [])
        .map((group) => group.filter((node) => node.nodeId !== -1))
        .filter((group) => group.length > 0);

      const cleanedOutNodeId = (form.outNodeId || []).filter((node) => node.nodeId !== -1);

      const inIpString = form.inIp
        .split("\n")
        .map((ip) => ip.trim())
        .filter((ip) => ip)
        .join(",");

      const data = {
        ...form,
        inIp: inIpString,
        outNodeId: cleanedOutNodeId,
        chainNodes: cleanedChainNodes,
      };

      const response = isEdit ? await updateTunnel(data) : await createTunnel(data);

      if (response.code === 0) {
        toast.success(isEdit ? "更新成功" : "创建成功");
        setModalOpen(false);
        loadData();
      } else {
        toast.error(response.msg || (isEdit ? "更新失败" : "创建失败"));
      }
    } catch (error) {
      console.error("提交失败:", error);
      toast.error("网络错误，请重试");
    } finally {
      setSubmitLoading(false);
    }
  };

  const handleDiagnose = async (tunnel: Tunnel) => {
    setCurrentDiagnosisTunnel(tunnel);
    setDiagnosisModalOpen(true);
    setDiagnosisLoading(true);
    setDiagnosisResult(null);

    try {
      const response = await diagnoseTunnel(tunnel.id);
      if (response.code === 0) {
        setDiagnosisResult(response.data);
      } else {
        toast.error(response.msg || "诊断失败");
        setDiagnosisResult({
          tunnelName: tunnel.name,
          tunnelType: tunnel.type === 1 ? "端口转发" : "隧道转发",
          timestamp: Date.now(),
          results: [
            {
              success: false,
              description: "诊断失败",
              nodeName: "-",
              nodeId: "-",
              targetIp: "-",
              targetPort: 443,
              message: response.msg || "诊断过程中发生错误",
            },
          ],
        });
      }
    } catch (error) {
      console.error("诊断失败:", error);
      toast.error("网络错误，请重试");
      setDiagnosisResult({
        tunnelName: tunnel.name,
        tunnelType: tunnel.type === 1 ? "端口转发" : "隧道转发",
        timestamp: Date.now(),
        results: [
          {
            success: false,
            description: "网络错误",
            nodeName: "-",
            nodeId: "-",
            targetIp: "-",
            targetPort: 443,
            message: "无法连接到服务器",
          },
        ],
      });
    } finally {
      setDiagnosisLoading(false);
    }
  };

  const getTypeDisplay = (type: number) => {
    switch (type) {
      case 1:
        return { text: "端口转发", tone: "cyan" as const };
      case 2:
        return { text: "隧道转发", tone: "violet" as const };
      default:
        return { text: "未知", tone: "slate" as const };
    }
  };

  const getFlowDisplay = (flow: number) => {
    switch (flow) {
      case 1:
        return "单向计算";
      case 2:
        return "双向计算";
      default:
        return "未知";
    }
  };

  const getQualityDisplay = (averageTime?: number, packetLoss?: number) => {
    if (averageTime === undefined || packetLoss === undefined) return null;

    if (averageTime < 30 && packetLoss === 0) return { text: "优秀", tone: "green" as const };
    if (averageTime < 50 && packetLoss === 0) return { text: "很好", tone: "green" as const };
    if (averageTime < 100 && packetLoss < 1) return { text: "良好", tone: "cyan" as const };
    if (averageTime < 150 && packetLoss < 2) return { text: "一般", tone: "amber" as const };
    if (averageTime < 200 && packetLoss < 5) return { text: "较差", tone: "amber" as const };
    return { text: "很差", tone: "red" as const };
  };

  const groupedDiagnosis = (results: DiagnosisResult["results"]) => {
    const entry = results.filter((r) => r.fromChainType === 1);
    const exit = results.filter((r) => r.fromChainType === 3);
    const chains: Record<number, typeof results> = {};
    const ungrouped = results.filter((r) => !r.fromChainType);
    results.forEach((r) => {
      if (r.fromChainType === 2 && r.fromInx != null) {
        if (!chains[r.fromInx]) chains[r.fromInx] = [];
        chains[r.fromInx].push(r);
      }
    });
    const rest = [...ungrouped];
    return { entry, chains, exit, rest };
  };

  const renderDiagSection = (title: string, results: DiagnosisResult["results"]) => {
    if (results.length === 0) return null;
    return (
      <div key={title} className="overflow-hidden rounded-2xl border border-white/[0.07]">
        <div className="border-b border-white/[0.07] bg-blue-400/[0.06] px-4 py-2">
          <h3 className="text-sm font-semibold text-blue-200">{title}</h3>
        </div>
        <div className="overflow-x-auto">
          <table className="flux-table min-w-[640px]">
            <thead>
              <tr>
                <th className="text-left">路径</th>
                <th className="text-center">状态</th>
                <th className="text-center">延迟(ms)</th>
                <th className="text-center">丢包率</th>
                <th className="text-center">质量</th>
              </tr>
            </thead>
            <tbody>
              {results.map((result, index) => {
                const quality = getQualityDisplay(result.averageTime, result.packetLoss);
                return (
                  <tr key={index}>
                    <td>
                      <div className="flex items-center gap-2">
                        <span
                          className={clsx(
                            "flex h-5 w-5 shrink-0 items-center justify-center rounded-full text-[11px] text-white",
                            result.success ? "bg-emerald-500" : "bg-red-500"
                          )}
                        >
                          {result.success ? "✓" : "✗"}
                        </span>
                        <div className="min-w-0">
                          <div className="truncate font-medium text-white">{result.description}</div>
                          <div className="truncate font-mono text-xs text-slate-500">
                            {result.targetIp}:{result.targetPort}
                          </div>
                          {!result.success && result.message && (
                            <div className="truncate text-xs text-red-300">{result.message}</div>
                          )}
                        </div>
                      </div>
                    </td>
                    <td className="text-center">
                      <Badge tone={result.success ? "green" : "red"}>
                        {result.success ? "成功" : "失败"}
                      </Badge>
                    </td>
                    <td className="text-center">
                      {result.success ? (
                        <span className="font-semibold text-blue-200">
                          {result.averageTime?.toFixed(0)}
                        </span>
                      ) : (
                        <span className="text-slate-500">-</span>
                      )}
                    </td>
                    <td className="text-center">
                      {result.success ? (
                        <span
                          className={clsx(
                            "font-semibold",
                            (result.packetLoss || 0) > 0 ? "text-amber-300" : "text-emerald-300"
                          )}
                        >
                          {result.packetLoss?.toFixed(1)}%
                        </span>
                      ) : (
                        <span className="text-slate-500">-</span>
                      )}
                    </td>
                    <td className="text-center">
                      {result.success && quality ? (
                        <Badge tone={quality.tone}>{quality.text}</Badge>
                      ) : (
                        <span className="text-slate-500">-</span>
                      )}
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      </div>
    );
  };

  const outProto =
    form.outNodeId && form.outNodeId.length > 0
      ? form.outNodeId[0].protocol || "tls"
      : "tls";
  const outStrategy =
    form.outNodeId && form.outNodeId.length > 0
      ? form.outNodeId[0].strategy || "round"
      : "round";

  if (loading) {
    return (
      <div>
        <PageHeader title="隧道管理" desc="管理端口转发与隧道转发拓扑" />
        <Card>
          <Loading text="正在加载..." />
        </Card>
      </div>
    );
  }

  return (
    <div>
      <PageHeader
        title="隧道管理"
        desc={tunnels.length > 0 ? `共 ${tunnels.length} 条隧道` : "管理端口转发与隧道转发拓扑"}
        actions={
          <Button variant="primary" size="sm" onClick={handleAdd}>
            <Plus size={15} /> 新增
          </Button>
        }
      />

      <Card>
        {tunnels.length === 0 ? (
          <Empty
            text="暂无隧道配置，点击右上按钮开始创建"
            action={
              <Button variant="primary" size="sm" onClick={handleAdd}>
                <Plus size={15} /> 新增隧道
              </Button>
            }
          />
        ) : (
          <div className="overflow-x-auto">
            <table className="flux-table min-w-[760px]" aria-label="隧道列表">
              <thead>
                <tr>
                  <th className="text-left">名称</th>
                  <th className="text-left">拓扑结构</th>
                  <th className="text-left">流量配置</th>
                  <th className="text-right">操作</th>
                </tr>
              </thead>
              <tbody>
                {tunnels.map((tunnel) => {
                  const td = getTypeDisplay(tunnel.type);
                  return (
                    <tr key={tunnel.id}>
                      <td>
                        <div className="flex items-center gap-2">
                          <span className="text-sm font-medium text-white">{tunnel.name}</span>
                          <Badge tone={td.tone}>{td.text}</Badge>
                        </div>
                      </td>
                      <td>
                        <div className="flex flex-wrap items-center gap-1.5 text-xs">
                          <span className="whitespace-nowrap rounded-lg border border-blue-400/25 bg-blue-400/10 px-2 py-0.5 font-medium text-blue-200">
                            {tunnel.inNodeId?.length || 0}入口
                          </span>
                          <span className="text-slate-500">→</span>
                          <span className="whitespace-nowrap rounded-lg border border-blue-500/30 bg-blue-500/15 px-2 py-0.5 font-medium text-blue-200">
                            {tunnel.type === 2 ? tunnel.chainNodes?.length || 0 : 0}跳
                          </span>
                          <span className="text-slate-500">→</span>
                          <span className="whitespace-nowrap rounded-lg border border-emerald-400/25 bg-emerald-400/10 px-2 py-0.5 font-medium text-emerald-200">
                            {tunnel.type === 2
                              ? tunnel.outNodeId?.length || 0
                              : tunnel.inNodeId?.length || 0}
                            出口
                          </span>
                        </div>
                      </td>
                      <td>
                        <div className="flex items-center gap-1.5">
                          <Badge tone="slate">{getFlowDisplay(tunnel.flow)}</Badge>
                          <Badge tone="amber">{tunnel.trafficRatio}x</Badge>
                        </div>
                      </td>
                      <td>
                        <div className="flex items-center justify-end gap-1.5">
                          <Button
                            size="sm"
                            variant="soft"
                            onClick={() => handleEdit(tunnel)}
                            title="编辑"
                          >
                            <Pencil size={14} />
                          </Button>
                          <Button
                            size="sm"
                            variant="outline"
                            onClick={() => handleDiagnose(tunnel)}
                            title="诊断"
                          >
                            <Stethoscope size={14} />
                          </Button>
                          <Button
                            size="sm"
                            variant="danger"
                            onClick={() => handleDelete(tunnel)}
                            title="删除"
                          >
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
        open={modalOpen}
        onClose={() => setModalOpen(false)}
        title={isEdit ? "编辑隧道" : "新增隧道"}
        desc={isEdit ? "修改配置后将实时下发到相关节点（隧道类型不可修改）" : "创建新的隧道配置"}
        width="max-w-3xl"
        footer={
          <>
            <Button variant="ghost" onClick={() => setModalOpen(false)}>
              取消
            </Button>
            <Button variant="primary" onClick={handleSubmit} loading={submitLoading}>
              {submitLoading ? (isEdit ? "更新中..." : "创建中...") : isEdit ? "更新" : "创建"}
            </Button>
          </>
        }
      >
        <div className="space-y-5">
          <Field label="隧道名称" hint={errors.name}>
            <Input
              placeholder="请输入隧道名称"
              value={form.name}
              onChange={(e) => setForm((prev) => ({ ...prev, name: e.target.value }))}
            />
          </Field>

          <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
            <Field label="隧道类型" hint={isEdit ? "编辑时无法修改隧道类型" : undefined}>
              <Select
                value={String(form.type)}
                disabled={isEdit}
                onChange={(e) => handleTypeChange(parseInt(e.target.value))}
              >
                <option value="1">端口转发</option>
                <option value="2">隧道转发</option>
              </Select>
            </Field>
            <Field label="状态">
              <Select
                value={String(form.status)}
                onChange={(e) => setForm((prev) => ({ ...prev, status: parseInt(e.target.value) }))}
              >
                <option value="1">启用</option>
                <option value="0">禁用</option>
              </Select>
            </Field>
          </div>

          <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
            <Field label="流量计算">
              <Select
                value={String(form.flow)}
                onChange={(e) => setForm((prev) => ({ ...prev, flow: parseInt(e.target.value) }))}
              >
                <option value="1">单向计算（仅上传）</option>
                <option value="2">双向计算（上传+下载）</option>
              </Select>
            </Field>
            <Field label="流量倍率" hint={errors.trafficRatio}>
              <Input
                type="number"
                step="0.1"
                placeholder="请输入流量倍率"
                value={String(form.trafficRatio)}
                onChange={(e) =>
                  setForm((prev) => ({ ...prev, trafficRatio: parseFloat(e.target.value) || 0 }))
                }
              />
            </Field>
          </div>

          <Field
            label="入口 IP"
            hint="支持多个 IP，每行一个地址，为空时使用入口节点 IP"
          >
            <Textarea
              placeholder={"一行一个IP地址或域名，例如:\n192.168.1.100\nexample.com"}
              value={form.inIp}
              rows={3}
              onChange={(e) => setForm((prev) => ({ ...prev, inIp: e.target.value }))}
            />
          </Field>

          <div className="border-t border-white/[0.07] pt-4">
            <h3 className="mb-1 text-sm font-semibold text-white">入口配置</h3>
            <p className="mb-2 text-xs text-slate-500">勾选入口节点，离线节点不可新选</p>
            <NodeChipMulti
              nodes={nodes}
              selected={inIds}
              onToggle={toggleInNode}
              isDisabled={(id) => {
                const n = nodeById(id);
                const selected = inIds.includes(id);
                if (n && n.status !== 1 && !selected) return { disabled: true, reason: "离线" };
                if (outIds.includes(id)) return { disabled: true, reason: "已选为出口" };
                if (chainIds.includes(id)) return { disabled: true, reason: "已选为转发链" };
                return { disabled: false };
              }}
            />
            {errors.inNodeId && <p className="mt-1 text-xs text-red-400">{errors.inNodeId}</p>}
            <div className="mt-2 flex flex-wrap gap-1.5">
              {form.inNodeId.map((ct) => {
                const n = nodeById(ct.nodeId);
                return (
                  <Badge key={ct.nodeId} tone="cyan">
                    {n?.name || `#${ct.nodeId}`}
                    <button
                      type="button"
                      className="ml-1 text-blue-300/70 hover:text-blue-100"
                      onClick={() => toggleInNode(ct.nodeId)}
                    >
                      <X size={11} />
                    </button>
                  </Badge>
                );
              })}
            </div>
          </div>

          {form.type === 2 && form.inNodeId.length > 0 && (
            <div className="border-t border-white/[0.07] pt-4">
              <h3 className="mb-1 text-sm font-semibold text-white">入口-出口绑定</h3>
              <p className="mb-3 text-xs text-slate-500">
                为每个入口节点指定它应该使用的出口节点。如果不指定，则使用所有出口节点。
              </p>
              <div className="space-y-3">
                {form.inNodeId.map((entry, entryIndex) => {
                  const entryNode = nodes.find((n) => n.id === entry.nodeId);
                  if (!entryNode) return null;
                  const sel = parseExitIds(entry);
                  return (
                    <div
                      key={entry.nodeId}
                      className="rounded-xl border border-white/[0.07] bg-white/[0.02] p-3"
                    >
                      <div className="mb-2 flex items-center justify-between">
                        <span className="text-sm font-medium text-slate-200">{entryNode.name}</span>
                        <Badge tone="cyan">入口</Badge>
                      </div>
                      <NodeChipMulti
                        nodes={(form.outNodeId || []).map((o) => nodeById(o.nodeId)).filter(Boolean) as Node[]}
                        selected={sel}
                        onToggle={(id) => {
                          setForm((prev) => {
                            const next = [...prev.inNodeId];
                            const cur = parseExitIds(next[entryIndex]);
                            const has = cur.includes(id);
                            const ids = has ? cur.filter((x) => x !== id) : [...cur, id];
                            next[entryIndex] = {
                              ...next[entryIndex],
                              exitNodeIds: ids.length > 0 ? ids : undefined,
                            };
                            return { ...prev, inNodeId: next };
                          });
                        }}
                        isDisabled={(id) => {
                          const n = nodeById(id);
                          if (n && n.status !== 1 && !sel.includes(id))
                            return { disabled: true, reason: "离线" };
                          return { disabled: false };
                        }}
                      />
                      {sel.length === 0 && (
                        <p className="mt-1.5 text-xs text-slate-500">未指定 = 使用全部出口节点</p>
                      )}
                    </div>
                  );
                })}
              </div>
            </div>
          )}

          {form.type === 2 && (
            <div className="border-t border-white/[0.07] pt-4">
              <div className="mb-3 flex items-center justify-between">
                <h3 className="text-sm font-semibold text-white">转发链配置</h3>
                <Button
                  size="sm"
                  variant="soft"
                  onClick={() => {
                    setForm((prev) => ({
                      ...prev,
                      chainNodes: [
                        ...(prev.chainNodes || []),
                        [{ nodeId: -1, chainType: 2, protocol: "tls", strategy: "round" }],
                      ],
                    }));
                  }}
                >
                  <Plus size={14} /> 添加一跳
                </Button>
              </div>

              {errors.chainNodes && (
                <p className="mb-2 text-xs text-red-400">{errors.chainNodes}</p>
              )}

              {getChainGroups().length > 0 ? (
                <div className="space-y-3">
                  {getChainGroups().map((groupNodes, groupIndex) => {
                    const real = groupNodes.filter((ct) => ct.nodeId !== -1);
                    const protocol = groupNodes.length > 0 ? groupNodes[0].protocol || "tls" : "tls";
                    const strategy = groupNodes.length > 0 ? groupNodes[0].strategy || "round" : "round";
                    const selectedHere = real.map((ct) => ct.nodeId);
                    const otherChainIds = (form.chainNodes || [])
                      .flatMap((group, idx) => (idx !== groupIndex ? group.map((ct) => ct.nodeId) : []))
                      .filter((id) => id !== -1);
                    return (
                      <div
                        key={groupIndex}
                        className="rounded-xl border border-white/[0.07] bg-white/[0.02] p-3"
                      >
                        <div className="mb-2 flex items-center justify-between">
                          <span className="flex items-center gap-1.5 text-sm font-medium text-slate-200">
                            <Route size={14} className="text-blue-300" />
                            第{groupIndex + 1}跳
                          </span>
                          <Button size="sm" variant="danger" onClick={() => removeChainNode(groupIndex)}>
                            <Trash2 size={13} /> 删除本跳
                          </Button>
                        </div>

                        <NodeChipMulti
                          nodes={nodes}
                          selected={selectedHere}
                          onToggle={(id) => toggleChainNode(groupIndex, id)}
                          isDisabled={(id) => {
                            const n = nodeById(id);
                            if (selectedHere.includes(id)) return { disabled: false };
                            if (n && n.status !== 1) return { disabled: true, reason: "离线" };
                            if (inIds.includes(id)) return { disabled: true, reason: "已选为入口" };
                            if (outIds.includes(id)) return { disabled: true, reason: "已选为出口" };
                            if (otherChainIds.includes(id)) return { disabled: true, reason: "已选为其他跳" };
                            return { disabled: false };
                          }}
                        />

                        <div className="mt-3 grid grid-cols-1 gap-3 md:grid-cols-2">
                          <Field label="协议">
                            <Select
                              value={protocol}
                              onChange={(e) => updateChainProtocol(groupIndex, e.target.value)}
                            >
                              {PROTOCOLS.map((p) => (
                                <option key={p} value={p}>
                                  {PROTOCOL_LABEL[p]}
                                </option>
                              ))}
                            </Select>
                          </Field>
                          <Field label="负载策略">
                            <Select
                              value={strategy}
                              onChange={(e) => updateChainStrategy(groupIndex, e.target.value)}
                            >
                              {STRATEGIES.map((s) => (
                                <option key={s.value} value={s.value}>
                                  {s.label}
                                </option>
                              ))}
                            </Select>
                          </Field>
                        </div>
                      </div>
                    );
                  })}
                </div>
              ) : (
                <div className="rounded-xl border border-dashed border-white/10 py-8 text-center">
                  <p className="text-xs text-slate-500">还没有添加转发链，点击上方“添加一跳”开始添加</p>
                </div>
              )}
            </div>
          )}

          {form.type === 2 && (
            <div className="border-t border-white/[0.07] pt-4">
              <h3 className="mb-1 text-sm font-semibold text-white">出口配置</h3>
              <p className="mb-2 text-xs text-slate-500">勾选出口节点，入口与转发链已选节点不可重复</p>
              <NodeChipMulti
                nodes={nodes}
                selected={outIds}
                onToggle={toggleOutNode}
                isDisabled={(id) => {
                  const n = nodeById(id);
                  const selected = outIds.includes(id);
                  if (n && n.status !== 1 && !selected) return { disabled: true, reason: "离线" };
                  if (!selected && inIds.includes(id)) return { disabled: true, reason: "已选为入口" };
                  if (!selected && chainIds.filter((x) => x !== -1).includes(id))
                    return { disabled: true, reason: "已选为转发链" };
                  return { disabled: false };
                }}
              />
              {errors.outNodeId && <p className="mt-1 text-xs text-red-400">{errors.outNodeId}</p>}
              <div className="mt-3 grid grid-cols-1 gap-3 md:grid-cols-2">
                <Field label="协议">
                  <Select value={outProto} onChange={(e) => setOutProtoStrategy("protocol", e.target.value)}>
                    {PROTOCOLS.map((p) => (
                      <option key={p} value={p}>
                        {PROTOCOL_LABEL[p]}
                      </option>
                    ))}
                  </Select>
                </Field>
                <Field label="负载策略">
                  <Select value={outStrategy} onChange={(e) => setOutProtoStrategy("strategy", e.target.value)}>
                    {STRATEGIES.map((s) => (
                      <option key={s.value} value={s.value}>
                        {s.label}
                      </option>
                    ))}
                  </Select>
                </Field>
              </div>
            </div>
          )}

          <Field label="启用状态">
            <div className="flex items-center gap-3">
              <Switch
                checked={form.status === 1}
                onChange={(v) => setForm((prev) => ({ ...prev, status: v ? 1 : 0 }))}
              />
              <span className="text-sm text-slate-300">{form.status === 1 ? "启用" : "禁用"}</span>
            </div>
          </Field>
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
          确定要删除隧道 <strong className="text-white">"{tunnelToDelete?.name}"</strong> 吗？
        </p>
      </Modal>

      <Modal
        open={diagnosisModalOpen}
        onClose={() => setDiagnosisModalOpen(false)}
        title="隧道诊断结果"
        desc={
          currentDiagnosisTunnel ? (
            <span className="inline-flex items-center gap-2">
              {currentDiagnosisTunnel.name}
              <Badge tone={currentDiagnosisTunnel.type === 1 ? "cyan" : "violet"}>
                {currentDiagnosisTunnel.type === 1 ? "端口转发" : "隧道转发"}
              </Badge>
            </span>
          ) : undefined
        }
        width="max-w-3xl"
        footer={
          <>
            <Button variant="ghost" onClick={() => setDiagnosisModalOpen(false)}>
              关闭
            </Button>
            {currentDiagnosisTunnel && (
              <Button
                variant="primary"
                onClick={() => currentDiagnosisTunnel && handleDiagnose(currentDiagnosisTunnel)}
                loading={diagnosisLoading}
              >
                重新诊断
              </Button>
            )}
          </>
        }
      >
        {diagnosisLoading ? (
          <Loading text="正在诊断..." />
        ) : diagnosisResult ? (
          <div className="space-y-4">
            <div className="grid grid-cols-3 gap-3">
              <div className="rounded-xl border border-white/[0.07] bg-white/[0.03] p-3 text-center">
                <div className="text-2xl font-bold text-white">{diagnosisResult.results.length}</div>
                <div className="mt-1 text-xs text-slate-500">总测试数</div>
              </div>
              <div className="rounded-xl border border-emerald-400/20 bg-emerald-400/[0.06] p-3 text-center">
                <div className="text-2xl font-bold text-emerald-300">
                  {diagnosisResult.results.filter((r) => r.success).length}
                </div>
                <div className="mt-1 text-xs text-emerald-300/80">成功</div>
              </div>
              <div className="rounded-xl border border-red-400/20 bg-red-400/[0.06] p-3 text-center">
                <div className="text-2xl font-bold text-red-300">
                  {diagnosisResult.results.filter((r) => !r.success).length}
                </div>
                <div className="mt-1 text-xs text-red-300/80">失败</div>
              </div>
            </div>

            {(() => {
              const g = groupedDiagnosis(diagnosisResult.results);
              return (
                <>
                  {renderDiagSection("入口测试", g.entry)}
                  {Object.keys(g.chains)
                    .map(Number)
                    .sort((a, b) => a - b)
                    .map((hop) => renderDiagSection(`转发链 · 第${hop}跳`, g.chains[hop]))}
                  {renderDiagSection("出口测试", g.exit)}
                  {renderDiagSection("其他", g.rest)}
                </>
              );
            })()}

            {diagnosisResult.results.some((r) => !r.success) && (
              <div className="space-y-2">
                <h4 className="text-sm font-semibold text-red-300">失败详情</h4>
                <div className="space-y-2">
                  {diagnosisResult.results
                    .filter((r) => !r.success)
                    .map((result, index) => (
                      <div
                        key={index}
                        className="rounded-xl border border-red-400/20 bg-red-400/[0.06] px-3 py-2 text-xs"
                      >
                        <div className="font-medium text-red-200">{result.description}</div>
                        <div className="mt-0.5 text-red-300/80">{result.message || "连接失败"}</div>
                      </div>
                    ))}
                </div>
              </div>
            )}
          </div>
        ) : (
          <Empty text="暂无诊断数据" />
        )}
      </Modal>
    </div>
  );
}
