import { useState, useEffect, useRef } from "react";
import toast from "react-hot-toast";
import {
  DndContext,
  closestCenter,
  KeyboardSensor,
  PointerSensor,
  useSensor,
  useSensors,
  DragEndEvent,
} from "@dnd-kit/core";
import {
  arrayMove,
  SortableContext,
  sortableKeyboardCoordinates,
  verticalListSortingStrategy,
  useSortable,
} from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";
import {
  Plus,
  Upload,
  Download,
  Pencil,
  Trash2,
  Stethoscope,
  GripVertical,
  Copy,
  Search,
  RefreshCw,
  ChevronLeft,
  ChevronRight,
  X,
} from "lucide-react";
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
  createForward,
  getForwardList,
  updateForward,
  deleteForward,
  forceDeleteForward,
  userTunnel,
  pauseForwardService,
  resumeForwardService,
  diagnoseForward,
  updateForwardOrder,
  batchDeleteForward,
  batchPauseForward,
  batchResumeForward,
  batchChangeTunnelForward,
} from "@/api";

interface Forward {
  id: number;
  name: string;
  tunnelId: number;
  tunnelName: string;
  inIp: string;
  inPort: number;
  remoteAddr: string;
  interfaceName?: string;
  strategy: string;
  status: number;
  inFlow: number;
  outFlow: number;
  serviceRunning: boolean;
  createdTime: string;
  userName?: string;
  userId?: number;
  inx?: number;
}

interface Tunnel {
  id: number;
  name: string;
  inNodePortSta?: number;
  inNodePortEnd?: number;
}

interface ForwardForm {
  id?: number;
  userId?: number;
  name: string;
  tunnelId: number | null;
  inPort: number | null;
  remoteAddr: string;
  interfaceName?: string;
  strategy: string;
}

interface AddressItem {
  id: number;
  address: string;
  copying: boolean;
}

interface DiagnosisResult {
  forwardName: string;
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

type Tone = "slate" | "green" | "red" | "amber" | "cyan" | "violet";

const statusTone = (status: number): { tone: Tone; text: string } => {
  switch (status) {
    case 1:
      return { tone: "green", text: "正常" };
    case 0:
      return { tone: "amber", text: "暂停" };
    case -1:
      return { tone: "red", text: "异常" };
    default:
      return { tone: "slate", text: "未知" };
  }
};

const strategyTone = (strategy: string): { tone: Tone; text: string } => {
  switch (strategy) {
    case "fifo":
      return { tone: "cyan", text: "主备" };
    case "round":
      return { tone: "green", text: "轮询" };
    case "rand":
      return { tone: "amber", text: "随机" };
    case "hash":
      return { tone: "violet", text: "哈希" };
    default:
      return { tone: "slate", text: "未知" };
  }
};

const qualityTone = (averageTime?: number, packetLoss?: number): { text: string; tone: Tone } | null => {
  if (averageTime === undefined || packetLoss === undefined) return null;
  if (averageTime < 30 && packetLoss === 0) return { text: "优秀", tone: "green" };
  if (averageTime < 50 && packetLoss === 0) return { text: "很好", tone: "green" };
  if (averageTime < 100 && packetLoss < 1) return { text: "良好", tone: "cyan" };
  if (averageTime < 150 && packetLoss < 2) return { text: "一般", tone: "amber" };
  if (averageTime < 200 && packetLoss < 5) return { text: "较差", tone: "amber" };
  return { text: "很差", tone: "red" };
};

const formatFlow = (value: number): string => {
  if (!value || value <= 0) return "0 B";
  if (value < 1024) return value + " B";
  const k = 1024;
  const sizes = ["B", "KB", "MB", "GB", "TB", "PB", "EB"];
  const i = Math.min(Math.floor(Math.log(value) / Math.log(k)), sizes.length - 1);
  return (value / Math.pow(k, i)).toFixed(2) + " " + sizes[i];
};

const formatInAddress = (ipString: string, port: number): string => {
  if (!ipString) return "";
  const items = ipString.split(",").map((item) => item.trim()).filter((item) => item);
  if (items.length === 0) return "";
  const firstItem = items[0];
  const hasPort = /:\d+$/.test(firstItem);
  if (hasPort) {
    if (items.length === 1) return items[0];
    return `${items[0]} (+${items.length - 1}个)`;
  }
  if (!port) return "";
  if (items.length === 1) {
    const ip = items[0];
    if (ip.includes(":") && !ip.startsWith("[")) return `[${ip}]:${port}`;
    return `${ip}:${port}`;
  }
  const firstIp = items[0];
  const formattedFirstIp = firstIp.includes(":") && !firstIp.startsWith("[") ? `[${firstIp}]` : firstIp;
  return `${formattedFirstIp}:${port} (+${items.length - 1}个)`;
};

const formatRemoteAddress = (addressString: string): string => {
  if (!addressString) return "";
  const addresses = addressString.split(",").map((addr) => addr.trim()).filter((addr) => addr);
  if (addresses.length === 0) return "";
  if (addresses.length === 1) return addresses[0];
  return `${addresses[0]} (+${addresses.length - 1})`;
};

const hasMultipleAddresses = (addressString: string): boolean => {
  if (!addressString) return false;
  const sep = addressString.includes("\n") ? "\n" : ",";
  const addresses = addressString.split(sep).map((addr) => addr.trim()).filter((addr) => addr);
  return addresses.length > 1;
};

const getAddressCount = (addressString: string): number => {
  if (!addressString) return 0;
  return addressString.split("\n").map((addr) => addr.trim()).filter((addr) => addr).length;
};

/* ---------- 可拖拽表格行 ---------- */
function SortableTableRow({
  forward,
  selected,
  onToggle,
  onToggleService,
  onEdit,
  onDiagnose,
  onDelete,
  onShowAddr,
}: {
  forward: Forward;
  selected: boolean;
  onToggle: () => void;
  onToggleService: () => void;
  onEdit: () => void;
  onDiagnose: () => void;
  onDelete: () => void;
  onShowAddr: (addr: string, port: number | null, title: string) => void;
}) {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({ id: forward.id });
  const st = statusTone(forward.status);
  const sg = strategyTone(forward.strategy);
  return (
    <tr
      ref={setNodeRef}
      style={{ transform: transform ? CSS.Transform.toString(transform) : undefined, transition: transition || undefined }}
      {...attributes}
      className={isDragging ? "opacity-30" : undefined}
    >
      <td className="w-10">
        <input
          type="checkbox"
          checked={selected}
          onChange={onToggle}
          className="h-4 w-4 cursor-pointer accent-blue-400"
          aria-label={`选择 ${forward.name}`}
        />
      </td>
      <td>
        <div className="flex items-center gap-2">
          <span
            className="cursor-grab touch-none p-1 text-slate-500 hover:text-slate-300 active:cursor-grabbing"
            {...listeners}
            title="拖拽排序"
          >
            <GripVertical size={15} />
          </span>
          <div className="min-w-0">
            <div className="truncate text-[0.86rem] font-medium text-white">{forward.name}</div>
            <div className="truncate text-[0.72rem] text-slate-500">{forward.tunnelName}</div>
          </div>
        </div>
      </td>
      <td>
        <span
          className={`font-mono text-[0.76rem] text-blue-300 ${hasMultipleAddresses(forward.inIp) ? "cursor-pointer hover:underline" : ""}`}
          onClick={() => hasMultipleAddresses(forward.inIp) && onShowAddr(forward.inIp, forward.inPort, "入口地址")}
          title={formatInAddress(forward.inIp, forward.inPort)}
        >
          {formatInAddress(forward.inIp, forward.inPort)}
        </span>
      </td>
      <td>
        <span
          className={`font-mono text-[0.76rem] text-blue-300 ${hasMultipleAddresses(forward.remoteAddr) ? "cursor-pointer hover:underline" : ""}`}
          onClick={() => hasMultipleAddresses(forward.remoteAddr) && onShowAddr(forward.remoteAddr, null, "目标地址")}
          title={formatRemoteAddress(forward.remoteAddr)}
        >
          {formatRemoteAddress(forward.remoteAddr)}
        </span>
      </td>
      <td>
        <div className="flex flex-wrap items-center gap-1.5">
          <Badge tone={sg.tone}>{sg.text}</Badge>
          <Badge tone="cyan">↑{formatFlow(forward.inFlow || 0)}</Badge>
          <Badge tone="green">↓{formatFlow(forward.outFlow || 0)}</Badge>
        </div>
      </td>
      <td>
        <div className="flex items-center gap-2">
          <Switch checked={forward.serviceRunning} disabled={forward.status !== 1 && forward.status !== 0} onChange={onToggleService} />
          <Badge tone={st.tone} dot={forward.status === 1}>{st.text}</Badge>
        </div>
      </td>
      <td>
        <div className="flex items-center justify-end gap-1.5">
          <Button size="icon" variant="soft" onClick={onEdit} title="编辑" aria-label={`编辑 ${forward.name}`}>
            <Pencil size={14} />
          </Button>
          <Button size="icon" variant="outline" onClick={onDiagnose} title="诊断" aria-label={`诊断 ${forward.name}`}>
            <Stethoscope size={14} />
          </Button>
          <Button size="icon" variant="danger" onClick={onDelete} title="删除" aria-label={`删除 ${forward.name}`}>
            <Trash2 size={14} />
          </Button>
        </div>
      </td>
    </tr>
  );
}

export default function ForwardPage() {
  const [loading, setLoading] = useState(true);
  const [pageLoading, setPageLoading] = useState(false);
  const [forwards, setForwards] = useState<Forward[]>([]);
  const [tunnels, setTunnels] = useState<Tunnel[]>([]);

  const [page, setPage] = useState(1);
  const [pageSize, setPageSize] = useState(20);
  const [total, setTotal] = useState(0);

  const [filterTunnelId, setFilterTunnelId] = useState<number | null>(null);
  const [filterStatus, setFilterStatus] = useState<number | null>(null);
  const [filterUserName, setFilterUserName] = useState("");

  const [forwardOrder, setForwardOrder] = useState<number[]>([]);

  const [modalOpen, setModalOpen] = useState(false);
  const [deleteModalOpen, setDeleteModalOpen] = useState(false);
  const [addressModalOpen, setAddressModalOpen] = useState(false);
  const [diagnosisModalOpen, setDiagnosisModalOpen] = useState(false);
  const [isEdit, setIsEdit] = useState(false);
  const [submitLoading, setSubmitLoading] = useState(false);
  const [deleteLoading, setDeleteLoading] = useState(false);
  const [diagnosisLoading, setDiagnosisLoading] = useState(false);
  const [forwardToDelete, setForwardToDelete] = useState<Forward | null>(null);
  const [currentDiagnosisForward, setCurrentDiagnosisForward] = useState<Forward | null>(null);
  const [diagnosisResult, setDiagnosisResult] = useState<DiagnosisResult | null>(null);
  const [addressModalTitle, setAddressModalTitle] = useState("");
  const [addressList, setAddressList] = useState<AddressItem[]>([]);

  const [exportModalOpen, setExportModalOpen] = useState(false);
  const [exportData, setExportData] = useState("");
  const [exportLoading, setExportLoading] = useState(false);
  const [selectedTunnelForExport, setSelectedTunnelForExport] = useState<number | null>(null);

  const [importModalOpen, setImportModalOpen] = useState(false);
  const [importData, setImportData] = useState("");
  const [importLoading, setImportLoading] = useState(false);
  const [selectedTunnelForImport, setSelectedTunnelForImport] = useState<number | null>(null);
  const [importResults, setImportResults] = useState<Array<{ line: string; success: boolean; message: string; forwardName?: string }>>([]);

  const [form, setForm] = useState<ForwardForm>({ name: "", tunnelId: null, inPort: null, remoteAddr: "", interfaceName: "", strategy: "fifo" });
  const [errors, setErrors] = useState<{ [key: string]: string }>({});

  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set());
  const [batchModalOpen, setBatchModalOpen] = useState(false);
  const [batchTargetTunnelId, setBatchTargetTunnelId] = useState<number | null>(null);
  const [batchLoading, setBatchLoading] = useState(false);

  const pageRef = useRef({ page: 1, pageSize: 20, tunnelId: null as number | null, status: null as number | null, keyword: "" });
  pageRef.current = { page, pageSize, tunnelId: filterTunnelId, status: filterStatus, keyword: filterUserName };
  const firstLoadRef = useRef(true);
  const skipKeywordEffect = useRef(true);

  useEffect(() => {
    loadTunnels();
    fetchForwards();
    skipKeywordEffect.current = false;
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    if (skipKeywordEffect.current) return;
    const t = setTimeout(() => {
      fetchForwards({ page: 1, keyword: filterUserName });
    }, 400);
    return () => clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [filterUserName]);

  const fetchForwards = async (opts?: {
    page?: number;
    pageSize?: number;
    tunnelId?: number | null;
    status?: number | null;
    keyword?: string;
    silent?: boolean;
  }) => {
    const p = opts?.page ?? pageRef.current.page;
    const ps = opts?.pageSize ?? pageRef.current.pageSize;
    const tid = opts?.tunnelId !== undefined ? opts.tunnelId : pageRef.current.tunnelId;
    const st = opts?.status !== undefined ? opts.status : pageRef.current.status;
    const kw = (opts?.keyword !== undefined ? opts.keyword : pageRef.current.keyword).trim();
    setPage(p);
    setPageSize(ps);
    if (!opts?.silent) {
      if (firstLoadRef.current) setLoading(true);
      else setPageLoading(true);
    }
    try {
      let cur = p;
      for (let attempt = 0; attempt < 2; attempt++) {
        const params: Record<string, unknown> = { page: cur, pageSize: ps };
        if (tid !== null && tid !== undefined) params.tunnelId = tid;
        if (st !== null && st !== undefined) params.status = st;
        if (kw) params.keyword = kw;
        const res = await getForwardList(params);
        if (res.code !== 0) {
          toast.error(res.msg || "获取转发列表失败");
          return;
        }
        const d: unknown = res.data;
        const rawList: unknown[] = Array.isArray(d) ? d : ((d as { list?: unknown[] })?.list || []);
        const mapped: Forward[] = rawList.map((f) => {
          const o = f as Record<string, unknown>;
          return { ...(o as object), serviceRunning: o.status === 1 } as Forward;
        });
        const tot: number = Array.isArray(d) ? mapped.length : ((d as { total?: number })?.total ?? mapped.length);
        if (mapped.length === 0 && cur > 1 && !Array.isArray(d)) {
          cur -= 1;
          setPage(cur);
          continue;
        }
        setForwards(mapped);
        setTotal(tot);
        setForwardOrder(mapped.map((f: Forward) => f.id));
        return;
      }
    } catch (error) {
      console.error("加载数据失败:", error);
      toast.error("加载数据失败");
    } finally {
      firstLoadRef.current = false;
      setLoading(false);
      setPageLoading(false);
    }
  };

  const loadData = async (lod = true) => {
    if (lod) await fetchForwards();
    else await fetchForwards({ silent: true });
  };

  const loadTunnels = async () => {
    try {
      const tunnelsRes = await userTunnel();
      if (tunnelsRes.code === 0) {
        setTunnels(tunnelsRes.data || []);
      } else {
        console.warn("获取隧道列表失败:", tunnelsRes.msg);
      }
    } catch (error) {
      console.warn("获取隧道列表失败:", error);
    }
  };

  const validateForm = (): boolean => {
    const newErrors: { [key: string]: string } = {};
    if (!form.name.trim()) {
      newErrors.name = "请输入转发名称";
    } else if (form.name.length < 2 || form.name.length > 50) {
      newErrors.name = "转发名称长度应在2-50个字符之间";
    }
    if (!form.tunnelId) {
      newErrors.tunnelId = "请选择关联隧道";
    }
    if (form.inPort !== null && form.inPort !== undefined) {
      const port = Number(form.inPort);
      if (isNaN(port) || port < 1 || port > 65535) {
        newErrors.inPort = "端口必须在 1-65535 之间";
      }
    }
    if (!form.remoteAddr.trim()) {
      newErrors.remoteAddr = "请输入远程地址";
    } else {
      const addresses = form.remoteAddr.split("\n").map((addr) => addr.trim()).filter((addr) => addr);
      const ipv4Pattern = /^(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?):\d+$/;
      const ipv6FullPattern = /^\[((([0-9a-fA-F]{1,4}:){7}([0-9a-fA-F]{1,4}|:))|(([0-9a-fA-F]{1,4}:){6}(:[0-9a-fA-F]{1,4}|((25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3})|:))|(([0-9a-fA-F]{1,4}:){5}(((:[0-9a-fA-F]{1,4}){1,2})|:((25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3})|:))|(([0-9a-fA-F]{1,4}:){4}(((:[0-9a-fA-F]{1,4}){1,3})|((:[0-9a-fA-F]{1,4})?:((25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}))|:))|(([0-9a-fA-F]{1,4}:){3}(((:[0-9a-fA-F]{1,4}){1,4})|((:[0-9a-fA-F]{1,4}){0,2}:((25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}))|:))|(([0-9a-fA-F]{1,4}:){2}(((:[0-9a-fA-F]{1,4}){1,5})|((:[0-9a-fA-F]{1,4}){0,3}:((25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}))|:))|(([0-9a-fA-F]{1,4}:){1}(((:[0-9a-fA-F]{1,4}){1,6})|((:[0-9a-fA-F]{1,4}){0,4}:((25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}))|:))|(:(((:[0-9a-fA-F]{1,4}){1,7})|((:[0-9a-fA-F]{1,4}){0,5}:((25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}))|:)))\]:\d+$/;
      const domainPattern = /^[a-zA-Z0-9]([a-zA-Z0-9\-]{0,61}[a-zA-Z0-9])?(\.[a-zA-Z0-9]([a-zA-Z0-9\-]{0,61}[a-zA-Z0-9])?)*:\d+$/;
      for (let i = 0; i < addresses.length; i++) {
        const addr = addresses[i];
        if (!ipv4Pattern.test(addr) && !ipv6FullPattern.test(addr) && !domainPattern.test(addr)) {
          newErrors.remoteAddr = `第${i + 1}行地址格式错误`;
          break;
        }
      }
    }
    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  };

  const handleAdd = () => {
    setIsEdit(false);
    setForm({ name: "", tunnelId: null, inPort: null, remoteAddr: "", interfaceName: "", strategy: "fifo" });
    setErrors({});
    setModalOpen(true);
  };

  const handleEdit = (forward: Forward) => {
    setIsEdit(true);
    setForm({
      id: forward.id,
      userId: forward.userId,
      name: forward.name,
      tunnelId: forward.tunnelId,
      inPort: forward.inPort,
      remoteAddr: forward.remoteAddr.split(",").join("\n"),
      interfaceName: forward.interfaceName || "",
      strategy: forward.strategy || "fifo",
    });
    setErrors({});
    setModalOpen(true);
  };

  const handleDelete = (forward: Forward) => {
    setForwardToDelete(forward);
    setDeleteModalOpen(true);
  };

  const confirmDelete = async () => {
    if (!forwardToDelete) return;
    setDeleteLoading(true);
    try {
      const res = await deleteForward(forwardToDelete.id);
      if (res.code === 0) {
        toast.success("删除成功");
        setDeleteModalOpen(false);
        loadData();
      } else {
        const confirmed = window.confirm(
          `常规删除失败：${res.msg || "删除失败"}\n\n是否需要强制删除？\n\n注意：强制删除不会去验证节点端是否已经删除对应的转发服务。`
        );
        if (confirmed) {
          const forceRes = await forceDeleteForward(forwardToDelete.id);
          if (forceRes.code === 0) {
            toast.success("强制删除成功");
            setDeleteModalOpen(false);
            loadData();
          } else {
            toast.error(forceRes.msg || "强制删除失败");
          }
        }
      }
    } catch (error) {
      console.error("删除失败:", error);
      toast.error("删除失败");
    } finally {
      setDeleteLoading(false);
    }
  };

  const handleTunnelChange = (tunnelId: string) => {
    setForm((prev) => {
      const newTunnelId = parseInt(tunnelId);
      if (prev.tunnelId === newTunnelId) return prev;
      if (isEdit) return { ...prev, tunnelId: newTunnelId };
      return { ...prev, tunnelId: newTunnelId, inPort: null };
    });
  };

  const syncForwardsSilently = async () => {
    await fetchForwards({ silent: true });
  };

  const handleSubmit = async () => {
    if (!validateForm()) return;
    setSubmitLoading(true);
    try {
      const processedRemoteAddr = form.remoteAddr
        .split("\n")
        .map((addr) => addr.trim())
        .filter((addr) => addr)
        .join(",");
      const addressCount = processedRemoteAddr.split(",").length;
      const finalStrategy = addressCount > 1 ? form.strategy : "fifo";
      let res;
      if (isEdit) {
        const updateData = {
          id: form.id,
          userId: form.userId,
          name: form.name,
          tunnelId: form.tunnelId,
          inPort: form.inPort,
          remoteAddr: processedRemoteAddr,
          strategy: finalStrategy,
        };
        res = await updateForward(updateData);
      } else {
        const createData = {
          name: form.name,
          tunnelId: form.tunnelId,
          inPort: form.inPort,
          remoteAddr: processedRemoteAddr,
          strategy: finalStrategy,
        };
        res = await createForward(createData);
      }
      if (res.code === 0) {
        toast.success(isEdit ? "修改成功" : "创建成功");
        setModalOpen(false);
        if (isEdit) syncForwardsSilently();
        else fetchForwards({ page: 1 });
      } else {
        toast.error(res.msg || "操作失败");
      }
    } catch (error) {
      console.error("提交失败:", error);
      toast.error("操作失败");
    } finally {
      setSubmitLoading(false);
    }
  };

  const handleServiceToggle = async (forward: Forward) => {
    if (forward.status !== 1 && forward.status !== 0) {
      toast.error("转发状态异常，无法操作");
      return;
    }
    const targetState = !forward.serviceRunning;
    try {
      setForwards((prev) => prev.map((f) => (f.id === forward.id ? { ...f, serviceRunning: targetState } : f)));
      let res;
      if (targetState) res = await resumeForwardService(forward.id);
      else res = await pauseForwardService(forward.id);
      if (res.code === 0) {
        toast.success(targetState ? "服务已启动" : "服务已暂停");
        setForwards((prev) => prev.map((f) => (f.id === forward.id ? { ...f, status: targetState ? 1 : 0 } : f)));
      } else {
        setForwards((prev) => prev.map((f) => (f.id === forward.id ? { ...f, serviceRunning: !targetState } : f)));
        toast.error(res.msg || "操作失败");
      }
    } catch (error) {
      setForwards((prev) => prev.map((f) => (f.id === forward.id ? { ...f, serviceRunning: !targetState } : f)));
      console.error("服务开关操作失败:", error);
      toast.error("网络错误，操作失败");
    }
  };

  const handleDiagnose = async (forward: Forward) => {
    setCurrentDiagnosisForward(forward);
    setDiagnosisModalOpen(true);
    setDiagnosisLoading(true);
    setDiagnosisResult(null);
    try {
      const response = await diagnoseForward(forward.id);
      if (response.code === 0) {
        setDiagnosisResult(response.data);
      } else {
        toast.error(response.msg || "诊断失败");
        setDiagnosisResult({
          forwardName: forward.name,
          timestamp: Date.now(),
          results: [
            {
              success: false,
              description: "诊断失败",
              nodeName: "-",
              nodeId: "-",
              targetIp: forward.remoteAddr.split(",")[0] || "-",
              message: response.msg || "诊断过程中发生错误",
            },
          ],
        });
      }
    } catch (error) {
      console.error("诊断失败:", error);
      toast.error("网络错误，请重试");
      setDiagnosisResult({
        forwardName: forward.name,
        timestamp: Date.now(),
        results: [
          {
            success: false,
            description: "网络错误",
            nodeName: "-",
            nodeId: "-",
            targetIp: forward.remoteAddr.split(",")[0] || "-",
            message: "无法连接到服务器",
          },
        ],
      });
    } finally {
      setDiagnosisLoading(false);
    }
  };

  const copyToClipboard = async (text: string, label = "内容") => {
    try {
      await navigator.clipboard.writeText(text);
      toast.success(`已复制${label}`);
    } catch {
      toast.error("复制失败");
    }
  };

  const showAddressModal = (addressString: string, port: number | null, title: string) => {
    if (!addressString) return;
    let addresses: string[];
    if (port !== null) {
      const items = addressString.split(",").map((item) => item.trim()).filter((item) => item);
      if (items.length <= 1) {
        copyToClipboard(formatInAddress(addressString, port), title);
        return;
      }
      const hasPort = /:\d+$/.test(items[0]);
      if (hasPort) {
        addresses = items;
      } else {
        addresses = items.map((ip) => (ip.includes(":") && !ip.startsWith("[") ? `[${ip}]:${port}` : `${ip}:${port}`));
      }
    } else {
      addresses = addressString.split(",").map((addr) => addr.trim()).filter((addr) => addr);
      if (addresses.length <= 1) {
        copyToClipboard(addressString, title);
        return;
      }
    }
    setAddressList(addresses.map((address, index) => ({ id: index, address, copying: false })));
    setAddressModalTitle(`${title} (${addresses.length}个)`);
    setAddressModalOpen(true);
  };

  const copyAddress = async (addressItem: AddressItem) => {
    try {
      setAddressList((prev) => prev.map((item) => (item.id === addressItem.id ? { ...item, copying: true } : item)));
      await copyToClipboard(addressItem.address, "地址");
    } catch {
      toast.error("复制失败");
    } finally {
      setAddressList((prev) => prev.map((item) => (item.id === addressItem.id ? { ...item, copying: false } : item)));
    }
  };

  const copyAllAddresses = async () => {
    if (addressList.length === 0) return;
    const allAddresses = addressList.map((item) => item.address).join("\n");
    await copyToClipboard(allAddresses, "所有地址");
  };

  const handleExport = () => {
    setSelectedTunnelForExport(null);
    setExportData("");
    setExportModalOpen(true);
  };

  const fetchAllFiltered = async (tunnelId: number | null): Promise<Forward[]> => {
    const all: Forward[] = [];
    const ps = 200;
    for (let p = 1; p <= 50; p++) {
      const params: Record<string, unknown> = { page: p, pageSize: ps };
      if (tunnelId !== null) params.tunnelId = tunnelId;
      if (filterStatus !== null) params.status = filterStatus;
      if (filterUserName.trim()) params.keyword = filterUserName.trim();
      const res = await getForwardList(params);
      if (res.code !== 0) throw new Error(res.msg || "获取转发列表失败");
      const d: unknown = res.data;
      const rawList: unknown[] = Array.isArray(d) ? d : ((d as { list?: unknown[] })?.list || []);
      const mapped: Forward[] = rawList.map((f) => {
        const o = f as Record<string, unknown>;
        return { ...(o as object), serviceRunning: o.status === 1 } as Forward;
      });
      all.push(...mapped);
      if (Array.isArray(d)) break;
      const tot: number = (d as { total?: number })?.total ?? mapped.length;
      if (all.length >= tot || mapped.length < ps) break;
    }
    return all;
  };

  const executeExport = async () => {
    if (!selectedTunnelForExport) {
      toast.error("请选择要导出的隧道");
      return;
    }
    setExportLoading(true);
    try {
      const forwardsToExport = await fetchAllFiltered(selectedTunnelForExport);
      if (forwardsToExport.length === 0) {
        toast.error("所选隧道没有转发数据");
        setExportLoading(false);
        return;
      }
      const exportLines = forwardsToExport.map((forward) => `${forward.remoteAddr}|${forward.name}|${forward.inPort}`);
      setExportData(exportLines.join("\n"));
    } catch (error) {
      console.error("导出失败:", error);
      toast.error("导出失败");
    } finally {
      setExportLoading(false);
    }
  };

  const copyExportData = async () => {
    await copyToClipboard(exportData, "转发数据");
  };

  const handleImport = () => {
    setImportData("");
    setImportResults([]);
    setSelectedTunnelForImport(null);
    setImportModalOpen(true);
  };

  const executeImport = async () => {
    if (!importData.trim()) {
      toast.error("请输入要导入的数据");
      return;
    }
    if (!selectedTunnelForImport) {
      toast.error("请选择要导入的隧道");
      return;
    }
    setImportLoading(true);
    setImportResults([]);
    try {
      const lines = importData.trim().split("\n").filter((line) => line.trim());
      type ImportResult = { line: string; success: boolean; message: string; forwardName?: string };
      const results: ImportResult[] = [];
      const runOne = async (rawLine: string): Promise<ImportResult> => {
        const line = rawLine.trim();
        const parts = line.split("|");
        if (parts.length < 2) return { line, success: false, message: "格式错误：需要至少包含目标地址和转发名称" };
        const [remoteAddr, name, inPort] = parts;
        if (!remoteAddr.trim() || !name.trim()) return { line, success: false, message: "目标地址和转发名称不能为空" };
        const addresses = remoteAddr.trim().split(",");
        const addressPattern = /^[^:]+:\d+$/;
        if (!addresses.every((addr) => addressPattern.test(addr.trim()))) {
          return { line, success: false, message: "目标地址格式错误，应为 地址:端口 格式，多个地址用逗号分隔" };
        }
        try {
          let portNumber: number | null = null;
          if (inPort && inPort.trim()) {
            const port = parseInt(inPort.trim());
            if (isNaN(port) || port < 1 || port > 65535) return { line, success: false, message: "入口端口格式错误，应为1-65535之间的数字" };
            portNumber = port;
          }
          const response = await createForward({
            name: name.trim(),
            tunnelId: selectedTunnelForImport,
            inPort: portNumber,
            remoteAddr: remoteAddr.trim(),
            strategy: "fifo",
          });
          if (response.code === 0) return { line, success: true, message: "创建成功", forwardName: name.trim() };
          return { line, success: false, message: response.msg || "创建失败" };
        } catch {
          return { line, success: false, message: "网络错误，创建失败" };
        }
      };
      const CONCURRENCY = 5;
      for (let i = 0; i < lines.length; i += CONCURRENCY) {
        const chunk = lines.slice(i, i + CONCURRENCY);
        const chunkResults = await Promise.all(chunk.map(runOne));
        results.push(...chunkResults);
      }
      setImportResults(results.reverse());
      toast.success("导入执行完成");
      await loadData(false);
    } catch (error) {
      console.error("导入失败:", error);
      toast.error("导入过程中发生错误");
    } finally {
      setImportLoading(false);
    }
  };

  const handleDragEnd = async (event: DragEndEvent) => {
    const { active, over } = event;
    if (!active || !over || active.id === over.id) return;
    if (total > pageSize || filterTunnelId !== null || filterStatus !== null || filterUserName.trim() !== "") {
      toast.error("多页或筛选模式下暂不支持拖拽排序，请清除筛选并将每页数量调大后重试");
      return;
    }
    if (!forwardOrder || forwardOrder.length === 0) return;
    const activeId = Number(active.id);
    const overId = Number(over.id);
    if (isNaN(activeId) || isNaN(overId)) return;
    const oldIndex = forwardOrder.indexOf(activeId);
    const newIndex = forwardOrder.indexOf(overId);
    if (oldIndex !== -1 && newIndex !== -1 && oldIndex !== newIndex) {
      const newOrder = arrayMove(forwardOrder, oldIndex, newIndex);
      setForwardOrder(newOrder);
      try {
        localStorage.setItem("forward-order", JSON.stringify(newOrder));
      } catch (error) {
        console.warn("无法保存排序到localStorage:", error);
      }
      try {
        const forwardsToUpdate = newOrder.map((id, index) => ({ id, inx: index }));
        const response = await updateForwardOrder({ forwards: forwardsToUpdate });
        if (response.code === 0) {
          setForwards((prev) =>
            prev.map((forward) => {
              const updatedForward = forwardsToUpdate.find((f) => f.id === forward.id);
              if (updatedForward) return { ...forward, inx: updatedForward.inx };
              return forward;
            })
          );
        } else {
          toast.error("保存排序失败：" + (response.msg || "未知错误"));
        }
      } catch (error) {
        console.error("保存排序到数据库失败:", error);
        toast.error("保存排序失败，请重试");
      }
    }
  };

  const toggleSelect = (id: number) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);
      if (next.has(id)) next.delete(id);
      else next.add(id);
      return next;
    });
  };

  const toggleSelectAll = () => {
    const currentIds = getSortedForwards().filter((f) => f && f.id).map((f) => f.id!);
    if (currentIds.every((id) => selectedIds.has(id))) setSelectedIds(new Set());
    else setSelectedIds(new Set(currentIds));
  };

  const handleBatchDelete = async () => {
    if (selectedIds.size === 0) return;
    if (!window.confirm(`确定删除选中的 ${selectedIds.size} 个转发吗？`)) return;
    setBatchLoading(true);
    try {
      const res = await batchDeleteForward(Array.from(selectedIds));
      if (res.code === 0) {
        toast.success(res.msg || `成功删除 ${selectedIds.size} 个转发`);
        setSelectedIds(new Set());
        loadData();
      } else {
        toast.error(res.msg || "批量删除失败");
        loadData();
      }
    } catch {
      toast.error("网络错误");
    } finally {
      setBatchLoading(false);
    }
  };

  const handleBatchPause = async () => {
    if (selectedIds.size === 0) return;
    if (!window.confirm(`确定暂停选中的 ${selectedIds.size} 个转发吗？`)) return;
    setBatchLoading(true);
    try {
      const res = await batchPauseForward(Array.from(selectedIds));
      if (res.code === 0) {
        toast.success(res.msg || `成功暂停 ${selectedIds.size} 个转发`);
        loadData();
      } else {
        toast.error(res.msg || "批量暂停失败");
        loadData();
      }
    } catch {
      toast.error("网络错误");
    } finally {
      setBatchLoading(false);
    }
  };

  const handleBatchResume = async () => {
    if (selectedIds.size === 0) return;
    if (!window.confirm(`确定恢复选中的 ${selectedIds.size} 个转发吗？`)) return;
    setBatchLoading(true);
    try {
      const res = await batchResumeForward(Array.from(selectedIds));
      if (res.code === 0) {
        toast.success(res.msg || `成功恢复 ${selectedIds.size} 个转发`);
        loadData();
      } else {
        toast.error(res.msg || "批量恢复失败");
        loadData();
      }
    } catch {
      toast.error("网络错误");
    } finally {
      setBatchLoading(false);
    }
  };

  const handleBatchChangeTunnel = () => {
    if (selectedIds.size === 0) return;
    setBatchTargetTunnelId(null);
    setBatchModalOpen(true);
  };

  const confirmBatchChangeTunnel = async () => {
    if (batchTargetTunnelId === null) {
      toast.error("请选择目标隧道");
      return;
    }
    setBatchLoading(true);
    try {
      const res = await batchChangeTunnelForward(Array.from(selectedIds), batchTargetTunnelId);
      if (res.code === 0) {
        toast.success(res.msg || `成功迁移 ${selectedIds.size} 个转发`);
        setSelectedIds(new Set());
        setBatchModalOpen(false);
        loadData();
      } else {
        toast.error(res.msg || "批量更改隧道失败");
        loadData();
      }
    } catch {
      toast.error("网络错误");
    } finally {
      setBatchLoading(false);
    }
  };

  const sensors = useSensors(useSensor(PointerSensor), useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }));

  const getFilteredForwards = (): Forward[] => {
    if (!forwards || forwards.length === 0) return [];
    return forwards;
  };

  const getSortedForwards = (): Forward[] => {
    const filteredForwards = getFilteredForwards();
    if (filteredForwards.length === 0) return [];
    const sortedForwards = [...filteredForwards].sort((a, b) => (a.inx ?? 0) - (b.inx ?? 0));
    if (forwardOrder && forwardOrder.length > 0 && sortedForwards.every((f) => f.inx === undefined || f.inx === 0)) {
      const forwardMap = new Map(filteredForwards.map((f) => [f.id, f]));
      const localSortedForwards: Forward[] = [];
      forwardOrder.forEach((id) => {
        const forward = forwardMap.get(id);
        if (forward) localSortedForwards.push(forward);
      });
      filteredForwards.forEach((forward) => {
        if (!forwardOrder.includes(forward.id)) localSortedForwards.push(forward);
      });
      return localSortedForwards;
    }
    return sortedForwards;
  };

  const sorted = getSortedForwards();
  const totalPages = Math.max(1, Math.ceil(total / pageSize));
  const allChecked = sorted.length > 0 && sorted.filter((f) => f && f.id).every((f) => selectedIds.has(f.id!));
  const pageNums = (() => {
    const arr: number[] = [];
    const start = Math.max(1, Math.min(page - 2, totalPages - 4));
    for (let i = start; i <= Math.min(totalPages, start + 4); i++) arr.push(i);
    return arr;
  })();

  const groupedDiagnosis = (results: DiagnosisResult["results"]) => {
    const entry = results.filter((r) => r.fromChainType === 1);
    const exit = results.filter((r) => r.fromChainType === 3);
    const chains: Record<number, DiagnosisResult["results"]> = {};
    results.forEach((r) => {
      if (r.fromChainType === 2 && r.fromInx != null) {
        if (!chains[r.fromInx]) chains[r.fromInx] = [];
        chains[r.fromInx].push(r);
      }
    });
    const other = results.filter((r) => r.fromChainType !== 1 && r.fromChainType !== 2 && r.fromChainType !== 3);
    return { entry, chains, exit, other };
  };

  if (loading) {
    return (
      <div>
        <PageHeader title="端口转发" desc="管理端口转发规则与目标地址" />
        <Card>
          <Loading text="正在加载…" />
        </Card>
      </div>
    );
  }

  return (
    <div className={selectedIds.size > 0 ? "pb-24" : undefined}>
      <PageHeader
        title="端口转发"
        desc={`共 ${total} 条转发规则 · 拖拽可排序${pageLoading ? " · 加载中…" : ""}`}
        actions={
          <>
            <Button size="sm" variant="outline" onClick={handleImport}>
              <Upload size={14} /> 导入
            </Button>
            <Button size="sm" variant="outline" onClick={handleExport} loading={exportLoading}>
              <Download size={14} /> 导出
            </Button>
            <Button size="sm" variant="primary" onClick={handleAdd}>
              <Plus size={14} /> 新建转发
            </Button>
          </>
        }
      />

      {/* 过滤条 */}
      <Card className="mb-4 p-4">
        <div className="flex flex-col gap-3 md:flex-row md:items-center">
          <div className="relative flex-1">
            <Search size={15} className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-slate-500" />
            <Input
              className="pl-9"
              placeholder="搜索名称 / 目标地址 / 用户…"
              value={filterUserName}
              onChange={(e) => setFilterUserName(e.target.value)}
              aria-label="搜索转发"
            />
          </div>
          <div className="flex gap-2">
            <Select
              className="w-44"
              value={filterTunnelId !== null ? String(filterTunnelId) : ""}
              onChange={(e) => {
                const v = e.target.value ? parseInt(e.target.value) : null;
                setFilterTunnelId(v);
                fetchForwards({ page: 1, tunnelId: v });
              }}
              aria-label="按隧道筛选"
            >
              <option value="">全部隧道</option>
              {tunnels.map((t) => (
                <option key={t.id} value={String(t.id)}>
                  {t.name}
                </option>
              ))}
            </Select>
            <Select
              className="w-32"
              value={filterStatus !== null ? String(filterStatus) : ""}
              onChange={(e) => {
                const v = e.target.value !== "" ? parseInt(e.target.value) : null;
                setFilterStatus(v);
                fetchForwards({ page: 1, status: v });
              }}
              aria-label="按状态筛选"
            >
              <option value="">全部状态</option>
              <option value="1">正常</option>
              <option value="0">暂停</option>
              <option value="-1">异常</option>
            </Select>
          </div>
        </div>
      </Card>

      {/* 批量操作条 */}
      {selectedIds.size > 0 && (
        <Card className="mb-4 border-blue-400/25 p-3">
          <div className="flex flex-wrap items-center gap-2">
            <span className="mr-1 text-[0.82rem] text-slate-300">
              已选择 <span className="font-semibold text-blue-300">{selectedIds.size}</span> 项
            </span>
            <Button size="sm" variant="danger" onClick={handleBatchDelete} loading={batchLoading}>
              批量删除
            </Button>
            <Button size="sm" variant="outline" onClick={handleBatchPause} loading={batchLoading}>
              批量暂停
            </Button>
            <Button size="sm" variant="soft" onClick={handleBatchResume} loading={batchLoading}>
              批量恢复
            </Button>
            <Button size="sm" variant="outline" onClick={handleBatchChangeTunnel} loading={batchLoading}>
              更改隧道
            </Button>
            <Button size="sm" variant="ghost" className="ml-auto" onClick={() => setSelectedIds(new Set())}>
              <X size={14} /> 取消选择
            </Button>
          </div>
        </Card>
      )}

      {/* 列表 */}
      <Card>
        {sorted.length > 0 ? (
          <>
            {/* 桌面端表格 */}
            <div className="hidden overflow-x-auto md:block">
              <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={handleDragEnd}>
                <SortableContext items={sorted.map((f) => f.id || 0).filter((id) => id > 0)} strategy={verticalListSortingStrategy}>
                  <table className="flux-table" aria-label="转发列表">
                    <thead>
                      <tr>
                        <th className="w-10 text-left">
                          <input type="checkbox" checked={allChecked} onChange={toggleSelectAll} className="h-4 w-4 cursor-pointer accent-blue-400" aria-label="全选" />
                        </th>
                        <th className="text-left">名称 / 隧道</th>
                        <th className="text-left">入口 inIp:inPort</th>
                        <th className="text-left">远端 remoteAddr</th>
                        <th className="text-left">策略 / 流量</th>
                        <th className="text-left">状态</th>
                        <th className="text-right">操作</th>
                      </tr>
                    </thead>
                    <tbody>
                      {sorted
                        .filter((f) => f && f.id)
                        .map((forward) => (
                          <SortableTableRow
                            key={forward.id!}
                            forward={forward}
                            selected={selectedIds.has(forward.id)}
                            onToggle={() => toggleSelect(forward.id)}
                            onToggleService={() => handleServiceToggle(forward)}
                            onEdit={() => handleEdit(forward)}
                            onDiagnose={() => handleDiagnose(forward)}
                            onDelete={() => handleDelete(forward)}
                            onShowAddr={showAddressModal}
                          />
                        ))}
                    </tbody>
                  </table>
                </SortableContext>
              </DndContext>
            </div>

            {/* 移动端卡片 */}
            <div className="space-y-3 p-4 md:hidden">
              {sorted
                .filter((f) => f && f.id)
                .map((forward) => {
                  const st = statusTone(forward.status);
                  const sg = strategyTone(forward.strategy);
                  const checked = selectedIds.has(forward.id);
                  return (
                    <div key={forward.id!} className={`rounded-xl border p-3.5 ${checked ? "border-blue-400/40 bg-blue-400/[0.06]" : "border-white/[0.08] bg-white/[0.03]"}`}>
                      <div className="flex items-start gap-2.5">
                        <input type="checkbox" checked={checked} onChange={() => toggleSelect(forward.id)} className="mt-1 h-4 w-4 shrink-0 accent-blue-400" aria-label={`选择 ${forward.name}`} />
                        <div className="min-w-0 flex-1">
                          <div className="truncate text-[0.9rem] font-semibold text-white">{forward.name}</div>
                          <div className="mt-0.5 truncate text-[0.74rem] text-slate-500">{forward.tunnelName}</div>
                        </div>
                        <Switch checked={forward.serviceRunning} disabled={forward.status !== 1 && forward.status !== 0} onChange={() => handleServiceToggle(forward)} />
                      </div>
                      <div className="mt-2.5 space-y-1.5 font-mono text-[0.74rem]">
                        <div className="truncate text-slate-400">
                          入口 <span className="text-blue-300">{formatInAddress(forward.inIp, forward.inPort)}</span>
                        </div>
                        <div className="break-all text-slate-400">
                          远端 <span className="text-blue-300">{formatRemoteAddress(forward.remoteAddr)}</span>
                        </div>
                      </div>
                      <div className="mt-2.5 flex flex-wrap items-center gap-1.5">
                        <Badge tone={sg.tone}>{sg.text}</Badge>
                        <Badge tone={st.tone} dot={forward.status === 1}>{st.text}</Badge>
                        <Badge tone="cyan">↑{formatFlow(forward.inFlow || 0)}</Badge>
                        <Badge tone="green">↓{formatFlow(forward.outFlow || 0)}</Badge>
                      </div>
                      <div className="mt-3 flex items-center gap-2 border-t border-white/[0.06] pt-3">
                        <Button size="sm" variant="soft" className="flex-1" onClick={() => handleEdit(forward)}>
                          <Pencil size={13} /> 编辑
                        </Button>
                        <Button size="sm" variant="outline" className="flex-1" onClick={() => handleDiagnose(forward)}>
                          <Stethoscope size={13} /> 诊断
                        </Button>
                        <Button size="sm" variant="danger" className="flex-1" onClick={() => handleDelete(forward)}>
                          <Trash2 size={13} /> 删除
                        </Button>
                      </div>
                    </div>
                  );
                })}
            </div>

            {/* 分页条 */}
            <div className="flex flex-col items-center justify-between gap-3 border-t border-white/[0.06] px-4 py-3.5 sm:flex-row">
              <div className="text-[0.76rem] text-slate-500">
                共 {total} 条{total > pageSize ? ` · 第 ${page} / ${totalPages} 页` : ""}
                {pageLoading && " · 加载中…"}
              </div>
              <div className="flex items-center gap-2.5">
                <Select
                  className="w-28"
                  value={String(pageSize)}
                  disabled={pageLoading}
                  onChange={(e) => fetchForwards({ page: 1, pageSize: parseInt(e.target.value) || 20 })}
                  aria-label="每页数量"
                >
                  <option value="10">10 / 页</option>
                  <option value="20">20 / 页</option>
                  <option value="50">50 / 页</option>
                  <option value="100">100 / 页</option>
                </Select>
                <div className="flex items-center gap-1">
                  <Button size="icon" variant="ghost" disabled={page <= 1 || pageLoading} onClick={() => fetchForwards({ page: page - 1 })} aria-label="上一页">
                    <ChevronLeft size={15} />
                  </Button>
                  {pageNums.map((p) => (
                    <button
                      key={p}
                      onClick={() => fetchForwards({ page: p })}
                      disabled={pageLoading}
                      className={`h-8 min-w-8 rounded-lg px-2 text-[0.8rem] font-medium transition-colors ${p === page ? "bg-blue-400/15 text-blue-200 border border-blue-400/25" : "text-slate-400 hover:bg-white/[0.06] hover:text-white"}`}
                    >
                      {p}
                    </button>
                  ))}
                  <Button size="icon" variant="ghost" disabled={page >= totalPages || pageLoading} onClick={() => fetchForwards({ page: page + 1 })} aria-label="下一页">
                    <ChevronRight size={15} />
                  </Button>
                </div>
              </div>
            </div>
          </>
        ) : (
          <Empty
            text="暂无转发配置，点击右上角新建开始创建"
            action={
              <Button size="sm" variant="primary" onClick={handleAdd}>
                <Plus size={14} /> 新建转发
              </Button>
            }
          />
        )}
      </Card>

      {/* 新建/编辑 */}
      <Modal
        open={modalOpen}
        onClose={() => setModalOpen(false)}
        title={isEdit ? "编辑转发" : "新建转发"}
        desc={isEdit ? "修改现有转发配置的信息" : "创建新的转发配置"}
        width="max-w-2xl"
        footer={
          <>
            <Button variant="ghost" onClick={() => setModalOpen(false)}>
              取消
            </Button>
            <Button variant="primary" onClick={handleSubmit} loading={submitLoading}>
              {isEdit ? "保存修改" : "创建转发"}
            </Button>
          </>
        }
      >
        <div className="space-y-4">
          <Field label="转发名称" hint={errors.name}>
            <Input placeholder="请输入转发名称（2-50 个字符）" value={form.name} onChange={(e) => setForm((prev) => ({ ...prev, name: e.target.value }))} />
          </Field>
          <Field label="关联隧道" hint={isEdit ? "修改隧道后将自动迁移转发服务，原入口端口可用时保持不变" : errors.tunnelId}>
            <Select value={form.tunnelId ? String(form.tunnelId) : ""} onChange={(e) => e.target.value && handleTunnelChange(e.target.value)}>
              <option value="" disabled>
                请选择关联的隧道
              </option>
              {tunnels.map((tunnel) => (
                <option key={tunnel.id} value={String(tunnel.id)}>
                  {tunnel.name}
                </option>
              ))}
            </Select>
            {!isEdit && errors.tunnelId && <span className="mt-1 block text-[0.72rem] text-red-400">{errors.tunnelId}</span>}
          </Field>
          <Field label="入口端口" hint="留空则从节点可用端口中自动分配">
            <Input
              type="number"
              placeholder="留空则自动分配可用端口"
              value={form.inPort !== null ? String(form.inPort) : ""}
              onChange={(e) => setForm((prev) => ({ ...prev, inPort: e.target.value ? parseInt(e.target.value) : null }))}
            />
            {errors.inPort && <span className="mt-1 block text-[0.72rem] text-red-400">{errors.inPort}</span>}
          </Field>
          <Field label="远程地址" hint="格式: IP:端口 或 域名:端口，多个地址每行一个">
            <Textarea
              rows={4}
              placeholder={"请输入远程地址，多个地址用换行分隔\n例如:\n192.168.1.100:8080\nexample.com:3000"}
              value={form.remoteAddr}
              onChange={(e) => setForm((prev) => ({ ...prev, remoteAddr: e.target.value }))}
            />
            {errors.remoteAddr && <span className="mt-1 block text-[0.72rem] text-red-400">{errors.remoteAddr}</span>}
          </Field>
          {getAddressCount(form.remoteAddr) > 1 && (
            <Field label="负载策略" hint="多个目标地址的负载均衡策略">
              <Select value={form.strategy} onChange={(e) => setForm((prev) => ({ ...prev, strategy: e.target.value }))}>
                <option value="fifo">主备模式 - 自上而下</option>
                <option value="round">轮询模式 - 依次轮换</option>
                <option value="rand">随机模式 - 随机选择</option>
                <option value="hash">哈希模式 - IP哈希</option>
              </Select>
            </Field>
          )}
        </div>
      </Modal>

      {/* 删除确认 */}
      <Modal
        open={deleteModalOpen}
        onClose={() => setDeleteModalOpen(false)}
        title="确认删除"
        desc="此操作无法撤销"
        footer={
          <>
            <Button variant="ghost" onClick={() => setDeleteModalOpen(false)}>
              取消
            </Button>
            <Button variant="danger" onClick={confirmDelete} loading={deleteLoading}>
              确认删除
            </Button>
          </>
        }
      >
        <p className="text-[0.86rem] text-slate-300">
          确定要删除转发 <span className="font-semibold text-white">“{forwardToDelete?.name}”</span> 吗？删除后该转发将永久消失。
        </p>
      </Modal>

      {/* 地址列表 */}
      <Modal open={addressModalOpen} onClose={() => setAddressModalOpen(false)} title={addressModalTitle} width="max-w-lg" footer={<Button variant="soft" onClick={copyAllAddresses}><Copy size={14} /> 复制全部</Button>}>
        <div className="max-h-60 space-y-2 overflow-y-auto">
          {addressList.map((item) => (
            <div key={item.id} className="flex items-center justify-between gap-3 rounded-xl border border-white/[0.08] bg-white/[0.03] p-3">
              <code className="mr-3 flex-1 break-all font-mono text-[0.8rem] text-slate-200">{item.address}</code>
              <Button size="sm" variant="ghost" loading={item.copying} onClick={() => copyAddress(item)}>
                <Copy size={13} /> 复制
              </Button>
            </div>
          ))}
        </div>
      </Modal>

      {/* 导出 */}
      <Modal
        open={exportModalOpen}
        onClose={() => {
          setExportModalOpen(false);
          setSelectedTunnelForExport(null);
          setExportData("");
        }}
        title="导出转发数据"
        desc="格式：目标地址|转发名称|入口端口"
        width="max-w-2xl"
        footer={
          <>
            <Button variant="ghost" onClick={() => setExportModalOpen(false)}>
              关闭
            </Button>
            {exportData && (
              <Button variant="soft" onClick={copyExportData}>
                <Copy size={14} /> 复制
              </Button>
            )}
          </>
        }
      >
        <div className="space-y-4">
          <Field label="选择导出隧道">
            <Select value={selectedTunnelForExport ? String(selectedTunnelForExport) : ""} onChange={(e) => setSelectedTunnelForExport(e.target.value ? parseInt(e.target.value) : null)}>
              <option value="" disabled>
                请选择要导出的隧道
              </option>
              {tunnels.map((tunnel) => (
                <option key={tunnel.id} value={String(tunnel.id)}>
                  {tunnel.name}
                </option>
              ))}
            </Select>
          </Field>
          <div className="text-right">
            <Button variant="primary" size="sm" onClick={executeExport} loading={exportLoading} disabled={!selectedTunnelForExport}>
              <Download size={14} /> {exportData ? "重新生成" : "生成导出数据"}
            </Button>
          </div>
          {exportData && <Textarea readOnly rows={10} value={exportData} className="font-mono text-[0.8rem]" placeholder="暂无数据" />}
        </div>
      </Modal>

      {/* 导入 */}
      <Modal
        open={importModalOpen}
        onClose={() => setImportModalOpen(false)}
        title="导入转发数据"
        desc="格式：目标地址|转发名称|入口端口，每行一个，入口端口留空将自动分配"
        width="max-w-2xl"
        footer={
          <>
            <Button variant="ghost" onClick={() => setImportModalOpen(false)}>
              关闭
            </Button>
            <Button variant="primary" onClick={executeImport} loading={importLoading} disabled={!importData.trim() || !selectedTunnelForImport}>
              <Upload size={14} /> 开始导入
            </Button>
          </>
        }
      >
        <div className="space-y-4">
          <p className="-mt-1 text-[0.76rem] text-slate-500">目标地址支持单个地址（如 example.com:8080）或多个地址用逗号分隔（如 3.3.3.3:3,4.4.4.4:4）</p>
          <Field label="选择导入隧道">
            <Select value={selectedTunnelForImport ? String(selectedTunnelForImport) : ""} onChange={(e) => setSelectedTunnelForImport(e.target.value ? parseInt(e.target.value) : null)}>
              <option value="" disabled>
                请选择要导入的隧道
              </option>
              {tunnels.map((tunnel) => (
                <option key={tunnel.id} value={String(tunnel.id)}>
                  {tunnel.name}
                </option>
              ))}
            </Select>
          </Field>
          <Field label="导入数据">
            <Textarea rows={8} placeholder="请输入要导入的转发数据，格式：目标地址|转发名称|入口端口" value={importData} onChange={(e) => setImportData(e.target.value)} className="font-mono text-[0.8rem]" />
          </Field>
          {importResults.length > 0 && (
            <div>
              <div className="mb-2 flex items-center justify-between">
                <h3 className="text-[0.9rem] font-semibold text-white">导入结果</h3>
                <span className="text-[0.76rem] text-slate-500">
                  成功：{importResults.filter((r) => r.success).length} / 总计：{importResults.length}
                </span>
              </div>
              <div className="max-h-40 space-y-1.5 overflow-y-auto">
                {importResults.map((result, index) => (
                  <div key={index} className={`rounded-lg border p-2 ${result.success ? "border-emerald-400/25 bg-emerald-400/[0.07]" : "border-red-400/25 bg-red-400/[0.07]"}`}>
                    <div className="flex items-center gap-2">
                      <span className={`text-[0.76rem] font-medium ${result.success ? "text-emerald-300" : "text-red-300"}`}>{result.success ? "成功" : "失败"}</span>
                      <span className="text-[0.76rem] text-slate-500">|</span>
                      <code className="truncate font-mono text-[0.74rem] text-slate-400">{result.line}</code>
                    </div>
                    <div className={`mt-0.5 text-[0.76rem] ${result.success ? "text-emerald-400/90" : "text-red-400/90"}`}>{result.message}</div>
                  </div>
                ))}
              </div>
            </div>
          )}
        </div>
      </Modal>

      {/* 诊断结果 */}
      <Modal
        open={diagnosisModalOpen}
        onClose={() => setDiagnosisModalOpen(false)}
        title="转发诊断结果"
        desc={currentDiagnosisForward ? `转发服务：${currentDiagnosisForward.name}` : undefined}
        width="max-w-4xl"
        footer={
          <>
            <Button variant="ghost" onClick={() => setDiagnosisModalOpen(false)}>
              关闭
            </Button>
            {currentDiagnosisForward && (
              <Button variant="primary" onClick={() => handleDiagnose(currentDiagnosisForward)} loading={diagnosisLoading}>
                <RefreshCw size={14} /> 重新诊断
              </Button>
            )}
          </>
        }
      >
        {diagnosisLoading ? (
          <Loading text="正在诊断…" />
        ) : diagnosisResult ? (
          <div className="space-y-4">
            <div className="grid grid-cols-3 gap-3">
              <div className="rounded-xl border border-white/[0.08] bg-white/[0.03] p-3 text-center">
                <div className="text-[1.4rem] font-bold text-white">{diagnosisResult.results.length}</div>
                <div className="mt-1 text-[0.74rem] text-slate-500">总测试数</div>
              </div>
              <div className="rounded-xl border border-emerald-400/25 bg-emerald-400/[0.07] p-3 text-center">
                <div className="text-[1.4rem] font-bold text-emerald-300">{diagnosisResult.results.filter((r) => r.success).length}</div>
                <div className="mt-1 text-[0.74rem] text-emerald-400/80">成功</div>
              </div>
              <div className="rounded-xl border border-red-400/25 bg-red-400/[0.07] p-3 text-center">
                <div className="text-[1.4rem] font-bold text-red-300">{diagnosisResult.results.filter((r) => !r.success).length}</div>
                <div className="mt-1 text-[0.74rem] text-red-400/80">失败</div>
              </div>
            </div>

            {(() => {
              const g = groupedDiagnosis(diagnosisResult.results);
              const sections: Array<{ title: string; list: DiagnosisResult["results"] }> = [
                { title: "入口测试", list: g.entry },
                ...Object.keys(g.chains)
                  .map(Number)
                  .sort((a, b) => a - b)
                  .map((hop) => ({ title: `转发链 - 第${hop}跳`, list: g.chains[hop] })),
                { title: "出口测试", list: g.exit },
                ...(g.other.length > 0 ? [{ title: "其他测试", list: g.other }] : []),
              ].filter((s) => s.list.length > 0);
              if (sections.length === 0 && diagnosisResult.results.length > 0) {
                sections.push({ title: "诊断明细", list: diagnosisResult.results });
              }
              return (
                <div className="space-y-3">
                  {sections.map((sec) => (
                    <div key={sec.title} className="overflow-hidden rounded-xl border border-white/[0.08]">
                      <div className="border-b border-white/[0.06] bg-blue-400/[0.07] px-3 py-2">
                        <h3 className="text-[0.82rem] font-semibold text-blue-200">{sec.title}</h3>
                      </div>
                      <div className="hidden overflow-x-auto md:block">
                        <table className="flux-table">
                          <thead>
                            <tr>
                              <th className="text-left">路径</th>
                              <th className="w-20 text-center">状态</th>
                              <th className="w-24 text-center">延迟(ms)</th>
                              <th className="w-24 text-center">丢包率</th>
                              <th className="w-20 text-center">质量</th>
                            </tr>
                          </thead>
                          <tbody>
                            {sec.list.map((result, index) => {
                              const q = qualityTone(result.averageTime, result.packetLoss);
                              return (
                                <tr key={index}>
                                  <td>
                                    <div className="flex items-center gap-2">
                                      <span className={`flex h-5 w-5 shrink-0 items-center justify-center rounded-full text-[0.7rem] ${result.success ? "bg-emerald-500 text-white" : "bg-red-500 text-white"}`}>
                                        {result.success ? "✓" : "✗"}
                                      </span>
                                      <div className="min-w-0 flex-1">
                                        <div className="truncate font-medium text-slate-100">{result.description}</div>
                                        <div className="truncate font-mono text-[0.72rem] text-slate-500">
                                          {result.targetIp}:{result.targetPort}
                                        </div>
                                      </div>
                                    </div>
                                  </td>
                                  <td className="text-center">
                                    <Badge tone={result.success ? "green" : "red"}>{result.success ? "成功" : "失败"}</Badge>
                                  </td>
                                  <td className="text-center">
                                    {result.success ? <span className="font-semibold text-blue-300">{result.averageTime?.toFixed(0)}</span> : <span className="text-slate-600">-</span>}
                                  </td>
                                  <td className="text-center">
                                    {result.success ? (
                                      <span className={`font-semibold ${(result.packetLoss || 0) > 0 ? "text-amber-300" : "text-emerald-300"}`}>{result.packetLoss?.toFixed(1)}%</span>
                                    ) : (
                                      <span className="text-slate-600">-</span>
                                    )}
                                  </td>
                                  <td className="text-center">
                                    {result.success && q ? <Badge tone={q.tone}>{q.text}</Badge> : <span className="text-slate-600">-</span>}
                                  </td>
                                </tr>
                              );
                            })}
                          </tbody>
                        </table>
                      </div>
                      <div className="space-y-2 p-3 md:hidden">
                        {sec.list.map((result, index) => {
                          const q = qualityTone(result.averageTime, result.packetLoss);
                          return (
                            <div key={index} className={`rounded-lg border p-3 ${result.success ? "border-white/[0.08] bg-white/[0.03]" : "border-red-400/25 bg-red-400/[0.06]"}`}>
                              <div className="mb-2 flex items-start gap-2">
                                <span className={`flex h-6 w-6 shrink-0 items-center justify-center rounded-full text-[0.72rem] ${result.success ? "bg-emerald-500 text-white" : "bg-red-500 text-white"}`}>
                                  {result.success ? "✓" : "✗"}
                                </span>
                                <div className="min-w-0 flex-1">
                                  <div className="break-words text-[0.84rem] font-semibold text-slate-100">{result.description}</div>
                                  <div className="mt-0.5 break-all font-mono text-[0.72rem] text-slate-500">
                                    {result.targetIp}:{result.targetPort}
                                  </div>
                                </div>
                                <Badge tone={result.success ? "green" : "red"}>{result.success ? "成功" : "失败"}</Badge>
                              </div>
                              {result.success ? (
                                <div className="grid grid-cols-3 gap-2 border-t border-white/[0.06] pt-2 text-center">
                                  <div>
                                    <div className="text-[1.05rem] font-bold text-blue-300">{result.averageTime?.toFixed(0)}</div>
                                    <div className="text-[0.72rem] text-slate-500">延迟(ms)</div>
                                  </div>
                                  <div>
                                    <div className={`text-[1.05rem] font-bold ${(result.packetLoss || 0) > 0 ? "text-amber-300" : "text-emerald-300"}`}>{result.packetLoss?.toFixed(1)}%</div>
                                    <div className="text-[0.72rem] text-slate-500">丢包率</div>
                                  </div>
                                  <div>
                                    {q && <Badge tone={q.tone}>{q.text}</Badge>}
                                    <div className="mt-0.5 text-[0.72rem] text-slate-500">质量</div>
                                  </div>
                                </div>
                              ) : (
                                <div className="border-t border-white/[0.06] pt-2 text-[0.76rem] text-red-300">{result.message || "连接失败"}</div>
                              )}
                            </div>
                          );
                        })}
                      </div>
                    </div>
                  ))}
                </div>
              );
            })()}

            {diagnosisResult.results.some((r) => !r.success) && (
              <div className="hidden space-y-2 md:block">
                <h4 className="text-[0.84rem] font-semibold text-red-300">失败详情</h4>
                <div className="space-y-2">
                  {diagnosisResult.results
                    .filter((r) => !r.success)
                    .map((result, index) => (
                      <div key={index} className="rounded-xl border border-red-400/25 bg-red-400/[0.06] p-3 text-[0.78rem]">
                        <div className="font-semibold text-red-200">{result.description}</div>
                        <div className="mt-1 text-red-300/80">{result.message || "连接失败"}</div>
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

      {/* 批量更改隧道 */}
      <Modal
        open={batchModalOpen}
        onClose={() => setBatchModalOpen(false)}
        title="批量更改隧道"
        desc={`将选中的 ${selectedIds.size} 个转发迁移到目标隧道`}
        footer={
          <>
            <Button variant="ghost" onClick={() => setBatchModalOpen(false)}>
              取消
            </Button>
            <Button variant="primary" onClick={confirmBatchChangeTunnel} loading={batchLoading}>
              确认迁移
            </Button>
          </>
        }
      >
        <Field label="目标隧道">
          <Select value={batchTargetTunnelId !== null ? String(batchTargetTunnelId) : ""} onChange={(e) => setBatchTargetTunnelId(e.target.value ? parseInt(e.target.value) : null)}>
            <option value="" disabled>
              请选择新的隧道
            </option>
            {tunnels.map((tunnel) => (
              <option key={tunnel.id} value={String(tunnel.id)}>
                {tunnel.name}
              </option>
            ))}
          </Select>
        </Field>
        <p className="mt-2 text-[0.74rem] text-slate-500">批量迁移将逐个处理每个转发，部分失败不影响其他转发。</p>
      </Modal>
    </div>
  );
}
