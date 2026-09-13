import { useState, useEffect } from "react";
import { Button } from "@heroui/button";
import { Input, Textarea } from "@heroui/input";
import { Select, SelectItem } from "@heroui/select";
import {
  Modal,
  ModalContent,
  ModalHeader,
  ModalBody,
  ModalFooter,
} from "@heroui/modal";
import { Chip } from "@heroui/chip";
import { Spinner } from "@heroui/spinner";
import { Switch } from "@heroui/switch";
import { Alert } from "@heroui/alert";
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
} from "@dnd-kit/sortable";
import { useSortable } from "@dnd-kit/sortable";
import { CSS } from "@dnd-kit/utilities";

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
import { JwtUtil } from "@/utils/jwt";

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
    fromChainType?: number; // 1: 入口, 2: 链, 3: 出口
    fromInx?: number;
    toChainType?: number;
    toInx?: number;
  }>;
}

export default function ForwardPage() {
  const [loading, setLoading] = useState(true);
  const [forwards, setForwards] = useState<Forward[]>([]);
  const [tunnels, setTunnels] = useState<Tunnel[]>([]);

  // 筛选状态
  const [filterTunnelId, setFilterTunnelId] = useState<number | null>(null);
  const [filterStatus, setFilterStatus] = useState<number | null>(null);
  const [filterUserName, setFilterUserName] = useState<string>("");

  // 检测是否为移动端
  const [isMobile, setIsMobile] = useState(false);

  useEffect(() => {
    const checkMobile = () => {
      setIsMobile(window.innerWidth < 768);
    };

    checkMobile();
    window.addEventListener("resize", checkMobile);

    return () => window.removeEventListener("resize", checkMobile);
  }, []);

  // 拖拽排序相关状态
  const [forwardOrder, setForwardOrder] = useState<number[]>([]);

  // 模态框状态
  const [modalOpen, setModalOpen] = useState(false);
  const [deleteModalOpen, setDeleteModalOpen] = useState(false);
  const [addressModalOpen, setAddressModalOpen] = useState(false);
  const [diagnosisModalOpen, setDiagnosisModalOpen] = useState(false);
  const [isEdit, setIsEdit] = useState(false);
  const [submitLoading, setSubmitLoading] = useState(false);
  const [deleteLoading, setDeleteLoading] = useState(false);
  const [diagnosisLoading, setDiagnosisLoading] = useState(false);
  const [forwardToDelete, setForwardToDelete] = useState<Forward | null>(null);
  const [currentDiagnosisForward, setCurrentDiagnosisForward] =
    useState<Forward | null>(null);
  const [diagnosisResult, setDiagnosisResult] =
    useState<DiagnosisResult | null>(null);
  const [addressModalTitle, setAddressModalTitle] = useState("");
  const [addressList, setAddressList] = useState<AddressItem[]>([]);

  // 导出相关状态
  const [exportModalOpen, setExportModalOpen] = useState(false);
  const [exportData, setExportData] = useState("");
  const [exportLoading, setExportLoading] = useState(false);
  const [selectedTunnelForExport, setSelectedTunnelForExport] = useState<
    number | null
  >(null);

  // 导入相关状态
  const [importModalOpen, setImportModalOpen] = useState(false);
  const [importData, setImportData] = useState("");
  const [importLoading, setImportLoading] = useState(false);
  const [selectedTunnelForImport, setSelectedTunnelForImport] = useState<
    number | null
  >(null);
  const [importResults, setImportResults] = useState<
    Array<{
      line: string;
      success: boolean;
      message: string;
      forwardName?: string;
    }>
  >([]);

  // 表单状态
  const [form, setForm] = useState<ForwardForm>({
    name: "",
    tunnelId: null,
    inPort: null,
    remoteAddr: "",
    interfaceName: "",
    strategy: "fifo",
  });

  // 表单验证错误
  const [errors, setErrors] = useState<{ [key: string]: string }>({});

  // 批量操作状态
  const [selectedIds, setSelectedIds] = useState<Set<number>>(new Set());
  const [batchModalOpen, setBatchModalOpen] = useState(false);
  const [batchTargetTunnelId, setBatchTargetTunnelId] = useState<number | null>(
    null,
  );
  const [batchLoading, setBatchLoading] = useState(false);

  useEffect(() => {
    loadData();
  }, []);

  // 加载所有数据
  const loadData = async (lod = true) => {
    setLoading(lod);
    try {
      const [forwardsRes, tunnelsRes] = await Promise.all([
        getForwardList(),
        userTunnel(),
      ]);

      if (forwardsRes.code === 0) {
        const forwardsData =
          forwardsRes.data?.map((forward: any) => ({
            ...forward,
            serviceRunning: forward.status === 1,
          })) || [];

        setForwards(forwardsData);

        // 初始化拖拽排序顺序
        const currentUserId = JwtUtil.getUserIdFromToken();
        let userForwards = forwardsData;

        if (currentUserId !== null) {
          userForwards = forwardsData.filter(
            (f: Forward) => f.userId === currentUserId,
          );
        }

        const hasDbOrdering = userForwards.some(
          (f: Forward) => f.inx !== undefined && f.inx !== 0,
        );

        if (hasDbOrdering) {
          const dbOrder = userForwards
            .sort((a: Forward, b: Forward) => (a.inx ?? 0) - (b.inx ?? 0))
            .map((f: Forward) => f.id);

          setForwardOrder(dbOrder);
          try {
            localStorage.setItem("forward-order", JSON.stringify(dbOrder));
          } catch (error) {
            console.warn("无法保存排序到localStorage:", error);
          }
        } else {
          const savedOrder = localStorage.getItem("forward-order");

          if (savedOrder) {
            try {
              const orderIds = JSON.parse(savedOrder);
              const validOrder = orderIds.filter((id: number) =>
                userForwards.some((f: Forward) => f.id === id),
              );

              userForwards.forEach((forward: Forward) => {
                if (!validOrder.includes(forward.id)) {
                  validOrder.push(forward.id);
                }
              });
              setForwardOrder(validOrder);
            } catch {
              setForwardOrder(userForwards.map((f: Forward) => f.id));
            }
          } else {
            setForwardOrder(userForwards.map((f: Forward) => f.id));
          }
        }
      } else {
        toast.error(forwardsRes.msg || "获取转发列表失败");
      }

      if (tunnelsRes.code === 0) {
        setTunnels(tunnelsRes.data || []);
      } else {
        console.warn("获取隧道列表失败:", tunnelsRes.msg);
      }
    } catch (error) {
      console.error("加载数据失败:", error);
      toast.error("加载数据失败");
    } finally {
      setLoading(false);
    }
  };

  // 表单验证
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

    // 验证入口端口（可选，如果填写则验证）
    if (form.inPort !== null && form.inPort !== undefined) {
      const port = Number(form.inPort);

      if (isNaN(port) || port < 1 || port > 65535) {
        newErrors.inPort = "端口必须在 1-65535 之间";
      }
    }

    if (!form.remoteAddr.trim()) {
      newErrors.remoteAddr = "请输入远程地址";
    } else {
      // 验证地址格式
      const addresses = form.remoteAddr
        .split("\n")
        .map((addr) => addr.trim())
        .filter((addr) => addr);
      const ipv4Pattern =
        /^(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?)\.(25[0-5]|2[0-4][0-9]|[01]?[0-9][0-9]?):\d+$/;
      const ipv6FullPattern =
        /^\[((([0-9a-fA-F]{1,4}:){7}([0-9a-fA-F]{1,4}|:))|(([0-9a-fA-F]{1,4}:){6}(:[0-9a-fA-F]{1,4}|((25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3})|:))|(([0-9a-fA-F]{1,4}:){5}(((:[0-9a-fA-F]{1,4}){1,2})|:((25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3})|:))|(([0-9a-fA-F]{1,4}:){4}(((:[0-9a-fA-F]{1,4}){1,3})|((:[0-9a-fA-F]{1,4})?:((25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}))|:))|(([0-9a-fA-F]{1,4}:){3}(((:[0-9a-fA-F]{1,4}){1,4})|((:[0-9a-fA-F]{1,4}){0,2}:((25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}))|:))|(([0-9a-fA-F]{1,4}:){2}(((:[0-9a-fA-F]{1,4}){1,5})|((:[0-9a-fA-F]{1,4}){0,3}:((25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}))|:))|(([0-9a-fA-F]{1,4}:){1}(((:[0-9a-fA-F]{1,4}){1,6})|((:[0-9a-fA-F]{1,4}){0,4}:((25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}))|:))|(:(((:[0-9a-fA-F]{1,4}){1,7})|((:[0-9a-fA-F]{1,4}){0,5}:((25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)(\.(25[0-5]|2[0-4]\d|1\d\d|[1-9]?\d)){3}))|:)))\]:\d+$/;
      const domainPattern =
        /^[a-zA-Z0-9]([a-zA-Z0-9\-]{0,61}[a-zA-Z0-9])?(\.[a-zA-Z0-9]([a-zA-Z0-9\-]{0,61}[a-zA-Z0-9])?)*:\d+$/;

      for (let i = 0; i < addresses.length; i++) {
        const addr = addresses[i];

        if (
          !ipv4Pattern.test(addr) &&
          !ipv6FullPattern.test(addr) &&
          !domainPattern.test(addr)
        ) {
          newErrors.remoteAddr = `第${i + 1}行地址格式错误`;
          break;
        }
      }
    }

    setErrors(newErrors);

    return Object.keys(newErrors).length === 0;
  };

  // 新增转发
  const handleAdd = () => {
    setIsEdit(false);
    setForm({
      name: "",
      tunnelId: null,
      inPort: null,
      remoteAddr: "",
      interfaceName: "",
      strategy: "fifo",
    });
    setErrors({});
    setModalOpen(true);
  };

  // 编辑转发
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

  // 显示删除确认
  const handleDelete = (forward: Forward) => {
    setForwardToDelete(forward);
    setDeleteModalOpen(true);
  };

  // 确认删除转发
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
        // 删除失败，询问是否强制删除
        const confirmed = window.confirm(
          `常规删除失败：${res.msg || "删除失败"}\n\n是否需要强制删除？\n\n⚠️ 注意：强制删除不会去验证节点端是否已经删除对应的转发服务。`,
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

  // 处理隧道选择变化：
  // - 新建：清空入口端口，由后端在新隧道上自动分配
  // - 编辑/改隧道：保留原入口端口，后端在目标隧道上尽量沿用（批量改隧道同逻辑）
  const handleTunnelChange = (tunnelId: string) => {
    setForm((prev) => {
      const newTunnelId = parseInt(tunnelId);

      if (prev.tunnelId === newTunnelId) return prev;
      if (isEdit) {
        return { ...prev, tunnelId: newTunnelId };
      }

      return { ...prev, tunnelId: newTunnelId, inPort: null };
    });
  };

  // 提交表单
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

      let res;

      if (isEdit) {
        // 更新时确保包含必要字段
        const updateData = {
          id: form.id,
          userId: form.userId,
          name: form.name,
          tunnelId: form.tunnelId,
          inPort: form.inPort,
          remoteAddr: processedRemoteAddr,
          strategy: addressCount > 1 ? form.strategy : "fifo",
        };

        res = await updateForward(updateData);
      } else {
        // 创建时不需要id和userId（后端会自动设置）
        const createData = {
          name: form.name,
          tunnelId: form.tunnelId,
          inPort: form.inPort,
          remoteAddr: processedRemoteAddr,
          strategy: addressCount > 1 ? form.strategy : "fifo",
        };

        res = await createForward(createData);
      }

      if (res.code === 0) {
        toast.success(isEdit ? "修改成功" : "创建成功");
        setModalOpen(false);
        loadData();
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

  // 处理服务开关
  const handleServiceToggle = async (forward: Forward) => {
    if (forward.status !== 1 && forward.status !== 0) {
      toast.error("转发状态异常，无法操作");

      return;
    }

    const targetState = !forward.serviceRunning;

    try {
      // 乐观更新UI
      setForwards((prev) =>
        prev.map((f) =>
          f.id === forward.id ? { ...f, serviceRunning: targetState } : f,
        ),
      );

      let res;

      if (targetState) {
        res = await resumeForwardService(forward.id);
      } else {
        res = await pauseForwardService(forward.id);
      }

      if (res.code === 0) {
        toast.success(targetState ? "服务已启动" : "服务已暂停");
        // 更新转发状态
        setForwards((prev) =>
          prev.map((f) =>
            f.id === forward.id ? { ...f, status: targetState ? 1 : 0 } : f,
          ),
        );
      } else {
        // 操作失败，恢复UI状态
        setForwards((prev) =>
          prev.map((f) =>
            f.id === forward.id ? { ...f, serviceRunning: !targetState } : f,
          ),
        );
        toast.error(res.msg || "操作失败");
      }
    } catch (error) {
      // 操作失败，恢复UI状态
      setForwards((prev) =>
        prev.map((f) =>
          f.id === forward.id ? { ...f, serviceRunning: !targetState } : f,
        ),
      );
      console.error("服务开关操作失败:", error);
      toast.error("网络错误，操作失败");
    }
  };

  // 诊断转发
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

  // 获取连接质量
  const getQualityDisplay = (averageTime?: number, packetLoss?: number) => {
    if (averageTime === undefined || packetLoss === undefined) return null;

    if (averageTime < 30 && packetLoss === 0)
      return { text: "🚀 优秀", color: "success" };
    if (averageTime < 50 && packetLoss === 0)
      return { text: "✨ 很好", color: "success" };
    if (averageTime < 100 && packetLoss < 1)
      return { text: "👍 良好", color: "primary" };
    if (averageTime < 150 && packetLoss < 2)
      return { text: "😐 一般", color: "warning" };
    if (averageTime < 200 && packetLoss < 5)
      return { text: "😟 较差", color: "warning" };

    return { text: "😵 很差", color: "danger" };
  };

  // 格式化流量
  const formatFlow = (value: number): string => {
    if (value === 0) return "0 B";
    if (value < 1024) return value + " B";
    if (value < 1024 * 1024) return (value / 1024).toFixed(2) + " KB";
    if (value < 1024 * 1024 * 1024)
      return (value / (1024 * 1024)).toFixed(2) + " MB";

    return (value / (1024 * 1024 * 1024)).toFixed(2) + " GB";
  };

  // 格式化入口地址
  const formatInAddress = (ipString: string, port: number): string => {
    if (!ipString) return "";

    const items = ipString
      .split(",")
      .map((item) => item.trim())
      .filter((item) => item);

    if (items.length === 0) return "";

    // 检查第一项是否已经包含端口（格式：IP:端口）
    const firstItem = items[0];
    const hasPort = /:\d+$/.test(firstItem);

    if (hasPort) {
      // inIp 已经包含完整的 IP:Port 组合
      if (items.length === 1) {
        return items[0];
      }

      return `${items[0]} (+${items.length - 1}个)`;
    }

    // inIp 只包含IP，需要添加端口（兼容旧数据）
    if (!port) return "";

    if (items.length === 1) {
      const ip = items[0];

      if (ip.includes(":") && !ip.startsWith("[")) {
        return `[${ip}]:${port}`;
      } else {
        return `${ip}:${port}`;
      }
    }

    const firstIp = items[0];
    let formattedFirstIp;

    if (firstIp.includes(":") && !firstIp.startsWith("[")) {
      formattedFirstIp = `[${firstIp}]`;
    } else {
      formattedFirstIp = firstIp;
    }

    return `${formattedFirstIp}:${port} (+${items.length - 1}个)`;
  };

  // 格式化远程地址
  const formatRemoteAddress = (addressString: string): string => {
    if (!addressString) return "";

    const addresses = addressString
      .split(",")
      .map((addr) => addr.trim())
      .filter((addr) => addr);

    if (addresses.length === 0) return "";
    if (addresses.length === 1) return addresses[0];

    return `${addresses[0]} (+${addresses.length - 1})`;
  };

  // 检查是否有多个地址
  const hasMultipleAddresses = (addressString: string): boolean => {
    if (!addressString) return false;
    const addresses = addressString
      .split(",")
      .map((addr) => addr.trim())
      .filter((addr) => addr);

    return addresses.length > 1;
  };

  // 显示地址列表弹窗
  const showAddressModal = (
    addressString: string,
    port: number | null,
    title: string,
  ) => {
    if (!addressString) return;

    let addresses: string[];

    if (port !== null) {
      // 入口地址处理
      const items = addressString
        .split(",")
        .map((item) => item.trim())
        .filter((item) => item);

      if (items.length <= 1) {
        copyToClipboard(formatInAddress(addressString, port), title);

        return;
      }

      // 检查是否已经包含端口
      const hasPort = /:\d+$/.test(items[0]);

      if (hasPort) {
        // 已经包含完整的 IP:Port 组合，直接使用
        addresses = items;
      } else {
        // 只包含IP，需要添加端口
        addresses = items.map((ip) => {
          if (ip.includes(":") && !ip.startsWith("[")) {
            return `[${ip}]:${port}`;
          } else {
            return `${ip}:${port}`;
          }
        });
      }
    } else {
      // 远程地址处理
      addresses = addressString
        .split(",")
        .map((addr) => addr.trim())
        .filter((addr) => addr);
      if (addresses.length <= 1) {
        copyToClipboard(addressString, title);

        return;
      }
    }

    setAddressList(
      addresses.map((address, index) => ({
        id: index,
        address,
        copying: false,
      })),
    );
    setAddressModalTitle(`${title} (${addresses.length}个)`);
    setAddressModalOpen(true);
  };

  // 复制到剪贴板
  const copyToClipboard = async (text: string, label: string = "内容") => {
    try {
      await navigator.clipboard.writeText(text);
      toast.success(`已复制${label}`);
    } catch (error) {
      toast.error("复制失败");
    }
  };

  // 复制地址
  const copyAddress = async (addressItem: AddressItem) => {
    try {
      setAddressList((prev) =>
        prev.map((item) =>
          item.id === addressItem.id ? { ...item, copying: true } : item,
        ),
      );
      await copyToClipboard(addressItem.address, "地址");
    } catch (error) {
      toast.error("复制失败");
    } finally {
      setAddressList((prev) =>
        prev.map((item) =>
          item.id === addressItem.id ? { ...item, copying: false } : item,
        ),
      );
    }
  };

  // 复制所有地址
  const copyAllAddresses = async () => {
    if (addressList.length === 0) return;
    const allAddresses = addressList.map((item) => item.address).join("\n");

    await copyToClipboard(allAddresses, "所有地址");
  };

  // 导出转发数据
  const handleExport = () => {
    setSelectedTunnelForExport(null);
    setExportData("");
    setExportModalOpen(true);
  };

  // 执行导出
  const executeExport = () => {
    if (!selectedTunnelForExport) {
      toast.error("请选择要导出的隧道");

      return;
    }

    setExportLoading(true);

    try {
      const forwardsToExport = getSortedForwards().filter(
        (forward) => forward.tunnelId === selectedTunnelForExport,
      );

      if (forwardsToExport.length === 0) {
        toast.error("所选隧道没有转发数据");
        setExportLoading(false);

        return;
      }

      // 格式化导出数据：remoteAddr|name|inPort
      const exportLines = forwardsToExport.map((forward) => {
        return `${forward.remoteAddr}|${forward.name}|${forward.inPort}`;
      });

      const exportText = exportLines.join("\n");

      setExportData(exportText);
    } catch (error) {
      console.error("导出失败:", error);
      toast.error("导出失败");
    } finally {
      setExportLoading(false);
    }
  };

  // 复制导出数据
  const copyExportData = async () => {
    await copyToClipboard(exportData, "转发数据");
  };

  // 导入转发数据
  const handleImport = () => {
    setImportData("");
    setImportResults([]);
    setSelectedTunnelForImport(null);
    setImportModalOpen(true);
  };

  // 执行导入
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
    setImportResults([]); // 清空之前的结果

    try {
      const lines = importData
        .trim()
        .split("\n")
        .filter((line) => line.trim());

      for (let i = 0; i < lines.length; i++) {
        const line = lines[i].trim();
        const parts = line.split("|");

        if (parts.length < 2) {
          setImportResults((prev) => [
            {
              line,
              success: false,
              message: "格式错误：需要至少包含目标地址和转发名称",
            },
            ...prev,
          ]);
          continue;
        }

        const [remoteAddr, name, inPort] = parts;

        if (!remoteAddr.trim() || !name.trim()) {
          setImportResults((prev) => [
            {
              line,
              success: false,
              message: "目标地址和转发名称不能为空",
            },
            ...prev,
          ]);
          continue;
        }

        // 验证远程地址格式 - 支持单个地址或多个地址用逗号分隔
        const addresses = remoteAddr.trim().split(",");
        const addressPattern = /^[^:]+:\d+$/;
        const isValidFormat = addresses.every((addr) =>
          addressPattern.test(addr.trim()),
        );

        if (!isValidFormat) {
          setImportResults((prev) => [
            {
              line,
              success: false,
              message:
                "目标地址格式错误，应为 地址:端口 格式，多个地址用逗号分隔",
            },
            ...prev,
          ]);
          continue;
        }

        try {
          // 处理入口端口
          let portNumber: number | null = null;

          if (inPort && inPort.trim()) {
            const port = parseInt(inPort.trim());

            if (isNaN(port) || port < 1 || port > 65535) {
              setImportResults((prev) => [
                {
                  line,
                  success: false,
                  message: "入口端口格式错误，应为1-65535之间的数字",
                },
                ...prev,
              ]);
              continue;
            }
            portNumber = port;
          }

          // 调用创建转发接口
          const response = await createForward({
            name: name.trim(),
            tunnelId: selectedTunnelForImport, // 使用用户选择的隧道
            inPort: portNumber, // 使用指定端口或自动分配
            remoteAddr: remoteAddr.trim(),
            strategy: "fifo",
          });

          if (response.code === 0) {
            setImportResults((prev) => [
              {
                line,
                success: true,
                message: "创建成功",
                forwardName: name.trim(),
              },
              ...prev,
            ]);
          } else {
            setImportResults((prev) => [
              {
                line,
                success: false,
                message: response.msg || "创建失败",
              },
              ...prev,
            ]);
          }
        } catch (error) {
          setImportResults((prev) => [
            {
              line,
              success: false,
              message: "网络错误，创建失败",
            },
            ...prev,
          ]);
        }
      }

      toast.success(`导入执行完成`);

      // 导入完成后刷新转发列表
      await loadData(false);
    } catch (error) {
      console.error("导入失败:", error);
      toast.error("导入过程中发生错误");
    } finally {
      setImportLoading(false);
    }
  };

  // 获取状态显示
  const getStatusDisplay = (status: number) => {
    switch (status) {
      case 1:
        return { color: "success", text: "正常" };
      case 0:
        return { color: "warning", text: "暂停" };
      case -1:
        return { color: "danger", text: "异常" };
      default:
        return { color: "default", text: "未知" };
    }
  };

  // 获取策略显示
  const getStrategyDisplay = (strategy: string) => {
    switch (strategy) {
      case "fifo":
        return { color: "primary", text: "主备" };
      case "round":
        return { color: "success", text: "轮询" };
      case "rand":
        return { color: "warning", text: "随机" };
      default:
        return { color: "default", text: "未知" };
    }
  };

  // 获取地址数量
  const getAddressCount = (addressString: string): number => {
    if (!addressString) return 0;
    const addresses = addressString
      .split("\n")
      .map((addr) => addr.trim())
      .filter((addr) => addr);

    return addresses.length;
  };

  // 处理拖拽结束
  const handleDragEnd = async (event: DragEndEvent) => {
    const { active, over } = event;

    if (!active || !over || active.id === over.id) return;

    // 确保 forwardOrder 存在且有效
    if (!forwardOrder || forwardOrder.length === 0) return;

    const activeId = Number(active.id);
    const overId = Number(over.id);

    // 检查 ID 是否有效
    if (isNaN(activeId) || isNaN(overId)) return;

    const oldIndex = forwardOrder.indexOf(activeId);
    const newIndex = forwardOrder.indexOf(overId);

    if (oldIndex !== -1 && newIndex !== -1 && oldIndex !== newIndex) {
      const newOrder = arrayMove(forwardOrder, oldIndex, newIndex);

      setForwardOrder(newOrder);

      // 保存到localStorage
      try {
        localStorage.setItem("forward-order", JSON.stringify(newOrder));
      } catch (error) {
        console.warn("无法保存排序到localStorage:", error);
      }

      // 持久化到数据库
      try {
        const forwardsToUpdate = newOrder.map((id, index) => ({
          id,
          inx: index,
        }));

        const response = await updateForwardOrder({
          forwards: forwardsToUpdate,
        });

        if (response.code === 0) {
          // 更新本地数据中的 inx 字段
          setForwards((prev) =>
            prev.map((forward) => {
              const updatedForward = forwardsToUpdate.find(
                (f) => f.id === forward.id,
              );

              if (updatedForward) {
                return { ...forward, inx: updatedForward.inx };
              }

              return forward;
            }),
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

  // 切换单个转发的选中状态
  const toggleSelect = (id: number) => {
    setSelectedIds((prev) => {
      const next = new Set(prev);

      if (next.has(id)) {
        next.delete(id);
      } else {
        next.add(id);
      }

      return next;
    });
  };

  // 全选/取消全选
  const toggleSelectAll = () => {
    const currentIds = getSortedForwards()
      .filter((f) => f && f.id)
      .map((f) => f.id!);

    if (currentIds.every((id) => selectedIds.has(id))) {
      setSelectedIds(new Set());
    } else {
      setSelectedIds(new Set(currentIds));
    }
  };

  // 批量删除
  const handleBatchDelete = async () => {
    if (selectedIds.size === 0) return;
    const confirmed = window.confirm(
      `确定删除选中的 ${selectedIds.size} 个转发吗？`,
    );

    if (!confirmed) return;
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
    } catch (error) {
      toast.error("网络错误");
    } finally {
      setBatchLoading(false);
    }
  };

  // 批量暂停
  const handleBatchPause = async () => {
    if (selectedIds.size === 0) return;
    const confirmed = window.confirm(
      `确定暂停选中的 ${selectedIds.size} 个转发吗？`,
    );

    if (!confirmed) return;
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
    } catch (error) {
      toast.error("网络错误");
    } finally {
      setBatchLoading(false);
    }
  };

  // 批量恢复
  const handleBatchResume = async () => {
    if (selectedIds.size === 0) return;
    const confirmed = window.confirm(
      `确定恢复选中的 ${selectedIds.size} 个转发吗？`,
    );

    if (!confirmed) return;
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
    } catch (error) {
      toast.error("网络错误");
    } finally {
      setBatchLoading(false);
    }
  };

  // 批量更改隧道
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
      const res = await batchChangeTunnelForward(
        Array.from(selectedIds),
        batchTargetTunnelId,
      );

      if (res.code === 0) {
        toast.success(res.msg || `成功迁移 ${selectedIds.size} 个转发`);
        setSelectedIds(new Set());
        setBatchModalOpen(false);
        loadData();
      } else {
        toast.error(res.msg || "批量更改隧道失败");
        loadData();
      }
    } catch (error) {
      toast.error("网络错误");
    } finally {
      setBatchLoading(false);
    }
  };
  const sensors = useSensors(
    useSensor(PointerSensor),
    useSensor(KeyboardSensor, {
      coordinateGetter: sortableKeyboardCoordinates,
    }),
  );

  // 获取筛选后的转发列表（已排序）
  const getFilteredForwards = (): Forward[] => {
    if (!forwards || forwards.length === 0) return [];

    let filtered = forwards;

    // 按用户筛选
    if (filterUserName.trim()) {
      filtered = filtered.filter((f) =>
        f.userName?.toLowerCase().includes(filterUserName.toLowerCase()),
      );
    }

    // 按隧道筛选
    if (filterTunnelId !== null) {
      filtered = filtered.filter((f) => f.tunnelId === filterTunnelId);
    }

    // 按状态筛选
    if (filterStatus !== null) {
      filtered = filtered.filter((f) => f.status === filterStatus);
    }

    return filtered;
  };

  // 根据排序顺序获取转发列表
  const getSortedForwards = (): Forward[] => {
    const filteredForwards = getFilteredForwards();

    if (filteredForwards.length === 0) return [];

    // 优先使用数据库中的 inx 字段进行排序
    const sortedForwards = [...filteredForwards].sort((a, b) => {
      const aInx = a.inx ?? 0;
      const bInx = b.inx ?? 0;

      return aInx - bInx;
    });

    // 如果数据库中没有排序信息，则使用本地存储的顺序
    if (
      forwardOrder &&
      forwardOrder.length > 0 &&
      sortedForwards.every((f) => f.inx === undefined || f.inx === 0)
    ) {
      const forwardMap = new Map(filteredForwards.map((f) => [f.id, f]));
      const localSortedForwards: Forward[] = [];

      forwardOrder.forEach((id) => {
        const forward = forwardMap.get(id);

        if (forward) {
          localSortedForwards.push(forward);
        }
      });

      filteredForwards.forEach((forward) => {
        if (!forwardOrder.includes(forward.id)) {
          localSortedForwards.push(forward);
        }
      });

      return localSortedForwards;
    }

    return sortedForwards;
  };

  // 可拖拽的转发 TableRow 组件（直接模式使用原生 <table> 以兼容 dnd-kit）
  const SortableTableRow = ({ forward }: { forward: Forward }) => {
    if (!forward || !forward.id) return null;

    const {
      attributes,
      listeners,
      setNodeRef,
      transform,
      transition,
      isDragging,
    } = useSortable({ id: forward.id });

    const cellClass = "px-3 py-2.5 text-sm";

    return (
      <tr
        ref={setNodeRef}
        style={{
          transform: transform ? CSS.Transform.toString(transform) : undefined,
          transition: transition || undefined,
        }}
        {...attributes}
        className={`${isDragging ? "opacity-30" : ""} bg-content1 border-b border-divider ${selectedIds.has(forward.id) ? "bg-primary-50/30" : ""}`}
      >
        <td className={`${cellClass} w-10`}>
          <input
            checked={selectedIds.has(forward.id)}
            className="w-4 h-4 rounded border-default-300 cursor-pointer accent-primary"
            type="checkbox"
            onChange={() => toggleSelect(forward.id)}
          />
        </td>
        <td className={cellClass}>
          <div className="flex items-center gap-2">
            <div
              className="cursor-grab active:cursor-grabbing p-1 text-default-400 hover:text-default-600 touch-manipulation flex-shrink-0"
              {...listeners}
              style={{ touchAction: "none" }}
              title={isMobile ? "长按拖拽排序" : "拖拽排序"}
            >
              <svg className="w-4 h-4" fill="currentColor" viewBox="0 0 20 20">
                <path d="M7 2a2 2 0 1 1 .001 4.001A2 2 0 0 1 7 2zm0 6a2 2 0 1 1 .001 4.001A2 2 0 0 1 7 8zm0 6a2 2 0 1 1 .001 4.001A2 2 0 0 1 7 14zm6-8a2 2 0 1 1-.001-4.001A2 2 0 0 1 13 6zm0 2a2 2 0 1 1 .001 4.001A2 2 0 0 1 13 8zm0 6a2 2 0 1 1 .001 4.001A2 2 0 0 1 13 14z" />
              </svg>
            </div>
            <div className="min-w-0">
              <div className="font-medium text-foreground text-sm truncate">
                {forward.name}
              </div>
              <div className="text-xs text-default-500 truncate">
                {forward.tunnelName}
              </div>
            </div>
          </div>
        </td>
        <td className={cellClass}>
          <span
            className={`text-xs font-mono text-primary ${hasMultipleAddresses(forward.inIp) ? 'cursor-pointer hover:underline' : ''}`}
            title={formatInAddress(forward.inIp, forward.inPort)}
            onClick={() => hasMultipleAddresses(forward.inIp) && showAddressModal(forward.inIp, forward.inPort, '入口端口')}
          >
            {formatInAddress(forward.inIp, forward.inPort)}
          </span>
        </td>
        <td className={cellClass}>
          <span
            className={`text-xs font-mono text-primary ${hasMultipleAddresses(forward.remoteAddr) ? 'cursor-pointer hover:underline' : ''}`}
            title={formatRemoteAddress(forward.remoteAddr)}
            onClick={() => hasMultipleAddresses(forward.remoteAddr) && showAddressModal(forward.remoteAddr, null, '目标地址')}
          >
            {formatRemoteAddress(forward.remoteAddr)}
          </span>
        </td>
        <td className={cellClass}>
          <div className="flex items-center gap-1">
            <Chip
              color={getStrategyDisplay(forward.strategy).color as any}
              variant="flat"
              size="sm"
              className="text-xs"
            >
              {getStrategyDisplay(forward.strategy).text}
            </Chip>
            <Chip className="text-xs" color="primary" size="sm" variant="flat">
              ↑{formatFlow(forward.inFlow || 0)}
            </Chip>
            <Chip className="text-xs" color="success" size="sm" variant="flat">
              ↓{formatFlow(forward.outFlow || 0)}
            </Chip>
          </div>
        </td>
        <td className={cellClass}>
          <div className="flex items-center gap-1.5">
            <Switch
              isDisabled={forward.status !== 1 && forward.status !== 0}
              isSelected={forward.serviceRunning}
              size="sm"
              onValueChange={() => handleServiceToggle(forward)}
            />
            <Chip
              color={getStatusDisplay(forward.status).color as any}
              variant="flat"
              size="sm"
              className="text-xs"
            >
              {getStatusDisplay(forward.status).text}
            </Chip>
          </div>
        </td>
        <td className={cellClass}>
          <div className="flex items-center gap-1 justify-end">
            <Button
              size="sm"
              variant="flat"
              color="primary"
              isIconOnly
              onPress={() => handleEdit(forward)}
            >
              <svg
                className="w-3.5 h-3.5"
                fill="currentColor"
                viewBox="0 0 20 20"
              >
                <path d="M13.586 3.586a2 2 0 112.828 2.828l-.793.793-2.828-2.828.793-.793zM11.379 5.793L3 14.172V17h2.828l8.38-8.379-2.83-2.828z" />
              </svg>
            </Button>
            <Button
              size="sm"
              variant="flat"
              color="warning"
              isIconOnly
              onPress={() => handleDiagnose(forward)}
            >
              <svg
                className="w-3.5 h-3.5"
                fill="currentColor"
                viewBox="0 0 20 20"
              >
                <path
                  fillRule="evenodd"
                  d="M8.257 3.099c.765-1.36 2.722-1.36 3.486 0l5.58 9.92c.75 1.334-.213 2.98-1.742 2.98H4.42c-1.53 0-2.493-1.646-1.743-2.98l5.58-9.92zM11 13a1 1 0 11-2 0 1 1 0 012 0zm-1-8a1 1 0 00-1 1v3a1 1 0 002 0V6a1 1 0 00-1-1z"
                  clipRule="evenodd"
                />
              </svg>
            </Button>
            <Button
              size="sm"
              variant="flat"
              color="danger"
              isIconOnly
              onPress={() => handleDelete(forward)}
            >
              <svg
                className="w-3.5 h-3.5"
                fill="currentColor"
                viewBox="0 0 20 20"
              >
                <path
                  fillRule="evenodd"
                  d="M9 2a1 1 0 000 2h2a1 1 0 100-2H9z"
                  clipRule="evenodd"
                />
                <path
                  fillRule="evenodd"
                  d="M10 18a8 8 0 100-16 8 8 0 000 16zM8 7a1 1 0 012 0v4a1 1 0 11-2 0V7zM12 7a1 1 0 012 0v4a1 1 0 11-2 0V7z"
                  clipRule="evenodd"
                />
              </svg>
            </Button>
          </div>
        </td>
      </tr>
    );
  };

  if (loading) {
    return (
      <div className="flex items-center justify-center h-64">
        <div className="flex items-center gap-3">
          <Spinner size="sm" />
          <span className="text-default-600">正在加载...</span>
        </div>
      </div>
    );
  }

  return (
    <div className={`px-3 lg:px-6 py-8 ${selectedIds.size > 0 ? "pb-20" : ""}`}>
      {/* 页面头部 */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 mb-6">
        {/* 筛选控件 */}
        <div className="flex items-center gap-2 flex-1 min-w-0">
          <Input
            size="sm"
            placeholder="搜索用户..."
            value={filterUserName}
            onChange={(e) => setFilterUserName(e.target.value)}
            className="w-36"
            variant="bordered"
            startContent={
              <svg
                className="w-3.5 h-3.5 text-default-400"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={2}
                  d="M21 21l-6-6m2-5a7 7 0 11-14 0 7 7 0 0114 0z"
                />
              </svg>
            }
          />
          <Select
            className="w-36"
              classNames={{ value: "text-xs" }}
              items={[{ id: 'all', name: '全部隧道' } as const, ...tunnels.map(t => ({ id: t.id.toString(), name: t.name }))]}
              placeholder="隧道"
              selectedKeys={filterTunnelId !== null ? [filterTunnelId.toString()] : []}
              size="sm"
              variant="bordered"
              onSelectionChange={(keys) => {
                const key = Array.from(keys)[0] as string | undefined;
                setFilterTunnelId(key && key !== 'all' ? parseInt(key) : null);
              }}
            ]}
          >
            {(item) => <SelectItem key={item.id}>{item.name}</SelectItem>}
          </Select>
          <Select
            size="sm"
            placeholder="状态"
            variant="bordered"
            selectedKeys={
              filterStatus !== null ? [filterStatus.toString()] : []
            }
            onSelectionChange={(keys) => {
              const key = Array.from(keys)[0] as string | undefined;

              setFilterStatus(key && key !== "all" ? parseInt(key) : null);
            }}
            className="w-28"
            classNames={{ value: "text-xs" }}
          >
            <SelectItem key="all">全部状态</SelectItem>
            <SelectItem key="1">正常</SelectItem>
            <SelectItem key="0">暂停</SelectItem>
            <SelectItem key="-1">异常</SelectItem>
          </Select>
        </div>
        <div className="flex items-center gap-3 flex-shrink-0">
          {/* 导入按钮 */}
          <Button
            size="sm"
            variant="flat"
            color="warning"
            onPress={handleImport}
          >
            导入
          </Button>

          {/* 导出按钮 */}
          <Button
            size="sm"
            variant="flat"
            color="success"
            onPress={handleExport}
            isLoading={exportLoading}
          >
            导出
          </Button>

          <Button size="sm" variant="flat" color="primary" onPress={handleAdd}>
            新增
          </Button>
        </div>
      </div>

      {/* 批量操作工具栏 - 固定在底部 */}
      {selectedIds.size > 0 && (
        <div className="fixed bottom-0 left-0 right-0 z-50 flex items-center gap-2 px-4 lg:px-6 py-3 bg-primary-50/95 border-t border-primary-200/50 shadow-lg backdrop-blur-sm">
          <span className="text-sm text-default-600 mr-1 whitespace-nowrap">
            已选择{" "}
            <span className="font-semibold text-primary">
              {selectedIds.size}
            </span>{" "}
            项
          </span>
          <div className="flex items-center gap-1.5">
            <Button
              size="sm"
              variant="flat"
              color="danger"
              onPress={handleBatchDelete}
              isLoading={batchLoading}
            >
              批量删除
            </Button>
            <Button
              size="sm"
              variant="flat"
              color="warning"
              onPress={handleBatchPause}
              isLoading={batchLoading}
            >
              批量暂停
            </Button>
            <Button
              size="sm"
              variant="flat"
              color="success"
              onPress={handleBatchResume}
              isLoading={batchLoading}
            >
              批量恢复
            </Button>
            <Button
              size="sm"
              variant="flat"
              color="primary"
              onPress={handleBatchChangeTunnel}
              isLoading={batchLoading}
            >
              更改隧道
            </Button>
          </div>
          <Button
            size="sm"
            variant="light"
            className="ml-auto"
            onPress={() => setSelectedIds(new Set())}
          >
            取消选择
          </Button>
        </div>
      )}

      {/* 转发列表（直接显示模式） */}
      {getSortedForwards().length > 0 ? (
        <DndContext
          collisionDetection={closestCenter}
            sensors={sensors}
            onDragEnd={handleDragEnd}
            onDragStart={() => {}}
        >
          <SortableContext
            items={getSortedForwards()
              .map((f) => f.id || 0)
              .filter((id) => id > 0)}
            strategy={verticalListSortingStrategy}
          >
            <div className="panel-table-wrap">
              <table aria-label="转发列表" className="w-full text-sm">
                <thead>
                  <tr className="bg-default-50 text-default-600 font-medium text-xs">
                    <th className="px-3 py-2.5 w-10 text-left">
                      <input
                        checked={getSortedForwards().length > 0 && getSortedForwards().filter(f => f && f.id).every(f => selectedIds.has(f.id!))}
                          className="w-4 h-4 rounded border-default-300 cursor-pointer accent-primary"
                          type="checkbox"
                          onChange={toggleSelectAll}
                      />
                    </th>
                    <th className="px-3 py-2.5 text-left">名称 / 隧道</th>
                    <th className="px-3 py-2.5 text-left">入口地址</th>
                    <th className="px-3 py-2.5 text-left">目标地址</th>
                    <th className="px-3 py-2.5 text-left">策略 / 流量</th>
                    <th className="px-3 py-2.5 text-left">状态</th>
                    <th className="px-3 py-2.5 text-right">操作</th>
                  </tr>
                </thead>
                <tbody className="divide-y divide-divider">
                  {getSortedForwards()
                    .filter((f) => f && f.id)
                    .map((forward) => (
                      <SortableTableRow key={forward.id!} forward={forward} />
                    ))}
                </tbody>
              </table>
            </div>
          </SortableContext>
        </DndContext>
      ) : (
        <div className="text-center py-16">
          <div className="flex flex-col items-center gap-4">
            <div className="w-16 h-16 bg-default-100 rounded-full flex items-center justify-center">
              <svg
                className="w-8 h-8 text-default-400"
                fill="none"
                stroke="currentColor"
                viewBox="0 0 24 24"
              >
                <path
                  strokeLinecap="round"
                  strokeLinejoin="round"
                  strokeWidth={1.5}
                  d="M8 9l4-4 4 4m0 6l-4 4-4-4"
                />
              </svg>
            </div>
            <div>
              <h3 className="text-lg font-semibold text-foreground">
                暂无转发配置
              </h3>
              <p className="text-default-500 text-sm mt-1">
                还没有创建任何转发配置，点击上方按钮开始创建
              </p>
            </div>
          </div>
        </div>
      )}

      {/* 新增/编辑模态框 */}
      <Modal
        backdrop="blur"
          isOpen={modalOpen}
          placement="center"
          scrollBehavior="outside"
          size="2xl"
          onOpenChange={setModalOpen}
      >
        <ModalContent>
          {(onClose) => (
            <>
              <ModalHeader className="flex flex-col gap-1">
                <h2 className="text-xl font-bold">
                  {isEdit ? "编辑转发" : "新增转发"}
                </h2>
                <p className="text-small text-default-500">
                  {isEdit ? "修改现有转发配置的信息" : "创建新的转发配置"}
                </p>
              </ModalHeader>
              <ModalBody>
                <div className="space-y-4 pb-4">
                  <Input
                    errorMessage={errors.name}
                      isInvalid={!!errors.name}
                      label="转发名称"
                      placeholder="请输入转发名称"
                      value={form.name}
                      variant="bordered"
                      onChange={(e) => setForm(prev => ({ ...prev, name: e.target.value }))}
                  />

                  <Select
                    label="选择隧道"
                    placeholder="请选择关联的隧道"
                    selectedKeys={
                      form.tunnelId ? [form.tunnelId.toString()] : []
                    }
                    onSelectionChange={(keys) => {
                      const selectedKey = Array.from(keys)[0] as string;

                      if (selectedKey) {
                        handleTunnelChange(selectedKey);
                      }
                    }}
                    isInvalid={!!errors.tunnelId}
                    errorMessage={errors.tunnelId}
                    variant="bordered"
                    description={
                      isEdit
                        ? "修改隧道后将自动迁移转发服务，原入口端口可用时保持不变"
                        : undefined
                    }
                  >
                    {tunnels.map((tunnel) => (
                      <SelectItem key={tunnel.id}>{tunnel.name}</SelectItem>
                    ))}
                  </Select>

                  <Input
                    description="指定入口端口，留空则从节点可用端口中自动分配"
                      errorMessage={errors.inPort}
                      isInvalid={!!errors.inPort}
                      label="入口端口"
                      placeholder="留空则自动分配可用端口"
                      type="number"
                      value={form.inPort !== null ? form.inPort.toString() : ''}
                      variant="bordered"
                      onChange={(e) => {
                        const value = e.target.value;
                        setForm(prev => ({ 
                          ...prev, 
                          inPort: value ? parseInt(value) : null
                        }));
                      }}
                  />

                  <Textarea
                    description="格式: IP:端口 或 域名:端口，支持多个地址（每行一个）"
                      errorMessage={errors.remoteAddr}
                      isInvalid={!!errors.remoteAddr}
                      label="远程地址"
                      maxRows={6}
                      minRows={3}
                      placeholder="请输入远程地址，多个地址用换行分隔&#10;例如:&#10;192.168.1.100:8080&#10;example.com:3000"
                      value={form.remoteAddr}
                      variant="bordered"
                      onChange={(e) => setForm(prev => ({ ...prev, remoteAddr: e.target.value }))}
                  />

                  {getAddressCount(form.remoteAddr) > 1 && (
                    <Select
                      label="负载策略"
                      placeholder="请选择负载均衡策略"
                      selectedKeys={[form.strategy]}
                      onSelectionChange={(keys) => {
                        const selectedKey = Array.from(keys)[0] as string;

                        setForm((prev) => ({ ...prev, strategy: selectedKey }));
                      }}
                      variant="bordered"
                      description="多个目标地址的负载均衡策略"
                    >
                      <SelectItem key="fifo">主备模式 - 自上而下</SelectItem>
                      <SelectItem key="round">轮询模式 - 依次轮换</SelectItem>
                      <SelectItem key="rand">随机模式 - 随机选择</SelectItem>
                      <SelectItem key="hash">哈希模式 - IP哈希</SelectItem>
                    </Select>
                  )}
                </div>
              </ModalBody>
              <ModalFooter>
                <Button variant="light" onPress={onClose}>
                  取消
                </Button>
                <Button
                  color="primary"
                  onPress={handleSubmit}
                  isLoading={submitLoading}
                >
                  {isEdit ? "保存修改" : "创建转发"}
                </Button>
              </ModalFooter>
            </>
          )}
        </ModalContent>
      </Modal>

      {/* 删除确认模态框 */}
      <Modal
        backdrop="blur"
          isOpen={deleteModalOpen}
          placement="center"
        scrollBehavior="outside"
        size="2xl"
        onOpenChange={setDeleteModalOpen}
      >
        <ModalContent>
          {(onClose) => (
            <>
              <ModalHeader className="flex flex-col gap-1">
                <h2 className="text-lg font-bold text-danger">确认删除</h2>
              </ModalHeader>
              <ModalBody>
                <p className="text-default-600">
                  确定要删除转发{" "}
                  <span className="font-semibold text-foreground">
                    "{forwardToDelete?.name}"
                  </span>{" "}
                  吗？
                </p>
                <p className="text-small text-default-500 mt-2">
                  此操作无法撤销，删除后该转发将永久消失。
                </p>
              </ModalBody>
              <ModalFooter>
                <Button variant="light" onPress={onClose}>
                  取消
                </Button>
                <Button
                  color="danger"
                  onPress={confirmDelete}
                  isLoading={deleteLoading}
                >
                  确认删除
                </Button>
              </ModalFooter>
            </>
          )}
        </ModalContent>
      </Modal>

      {/* 地址列表弹窗 */}
      <Modal
        isOpen={addressModalOpen}
        onClose={() => setAddressModalOpen(false)}
        size="lg"
        scrollBehavior="outside"
      >
        <ModalContent>
          <ModalHeader className="text-base">{addressModalTitle}</ModalHeader>
          <ModalBody className="pb-6">
            <div className="mb-4 text-right">
              <Button size="sm" onClick={copyAllAddresses}>
                复制
              </Button>
            </div>

            <div className="space-y-2 max-h-60 overflow-y-auto">
              {addressList.map((item) => (
                <div
                  key={item.id}
                  className="flex justify-between items-center p-3 border border-default-200 dark:border-default-100 rounded-lg"
                >
                  <code className="text-sm flex-1 mr-3 text-foreground">
                    {item.address}
                  </code>
                  <Button
                    size="sm"
                    variant="light"
                    isLoading={item.copying}
                    onClick={() => copyAddress(item)}
                  >
                    复制
                  </Button>
                </div>
              ))}
            </div>
          </ModalBody>
        </ModalContent>
      </Modal>

      {/* 导出数据模态框 */}
      <Modal
        isOpen={exportModalOpen}
        onClose={() => {
          setExportModalOpen(false);
          setSelectedTunnelForExport(null);
          setExportData("");
        }}
        size="2xl"
        scrollBehavior="outside"
        backdrop="blur"
        placement="center"
      >
        <ModalContent>
          <ModalHeader className="flex flex-col gap-1">
            <h2 className="text-xl font-bold">导出转发数据</h2>
            <p className="text-small text-default-500">
              格式：目标地址|转发名称|入口端口
            </p>
          </ModalHeader>
          <ModalBody className="pb-6">
            <div className="space-y-4">
              {/* 隧道选择 */}
              <div>
                <Select
                  isRequired
                    label="选择导出隧道"
                    placeholder="请选择要导出的隧道"
                    selectedKeys={selectedTunnelForExport ? [selectedTunnelForExport.toString()] : []}
                    variant="bordered"
                    onSelectionChange={(keys) => {
                      const selectedKey = Array.from(keys)[0] as string;
                      setSelectedTunnelForExport(selectedKey ? parseInt(selectedKey) : null);
                    }}
                >
                  {tunnels.map((tunnel) => (
                    <SelectItem
                      key={tunnel.id.toString()}
                      textValue={tunnel.name}
                    >
                      {tunnel.name}
                    </SelectItem>
                  ))}
                </Select>
              </div>

              {/* 导出按钮和数据 */}
              {exportData && (
                <div className="flex justify-between items-center">
                  <Button
                    color="primary"
                    size="sm"
                    onPress={executeExport}
                    isLoading={exportLoading}
                    isDisabled={!selectedTunnelForExport}
                    startContent={
                      <svg
                        className="w-4 h-4"
                        fill="currentColor"
                        viewBox="0 0 20 20"
                      >
                        <path
                          fillRule="evenodd"
                          d="M3 17a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zM6.293 6.707a1 1 0 010-1.414l3-3a1 1 0 011.414 0l3 3a1 1 0 01-1.414 1.414L11 5.414V13a1 1 0 11-2 0V5.414L7.707 6.707a1 1 0 01-1.414 0z"
                          clipRule="evenodd"
                        />
                      </svg>
                    }
                  >
                    重新生成
                  </Button>
                  <Button
                    color="secondary"
                    size="sm"
                    onPress={copyExportData}
                    startContent={
                      <svg
                        className="w-4 h-4"
                        fill="currentColor"
                        viewBox="0 0 20 20"
                      >
                        <path d="M8 3a1 1 0 011-1h2a1 1 0 110 2H9a1 1 0 01-1-1z" />
                        <path d="M6 3a2 2 0 00-2 2v11a2 2 0 002 2h8a2 2 0 002-2V5a2 2 0 00-2-2 3 3 0 01-3 3H9a3 3 0 01-3-3z" />
                      </svg>
                    }
                  >
                    复制
                  </Button>
                </div>
              )}

              {/* 初始导出按钮 */}
              {!exportData && (
                <div className="text-right">
                  <Button
                    color="primary"
                    size="sm"
                    onPress={executeExport}
                    isLoading={exportLoading}
                    isDisabled={!selectedTunnelForExport}
                    startContent={
                      <svg
                        className="w-4 h-4"
                        fill="currentColor"
                        viewBox="0 0 20 20"
                      >
                        <path
                          fillRule="evenodd"
                          d="M3 17a1 1 0 011-1h12a1 1 0 110 2H4a1 1 0 01-1-1zM6.293 6.707a1 1 0 010-1.414l3-3a1 1 0 011.414 0l3 3a1 1 0 01-1.414 1.414L11 5.414V13a1 1 0 11-2 0V5.414L7.707 6.707a1 1 0 01-1.414 0z"
                          clipRule="evenodd"
                        />
                      </svg>
                    }
                  >
                    生成导出数据
                  </Button>
                </div>
              )}

              {/* 导出数据显示 */}
              {exportData && (
                <div className="relative">
                  <Textarea
                    value={exportData}
                    readOnly
                    variant="bordered"
                    minRows={10}
                    maxRows={20}
                    className="font-mono text-sm"
                    classNames={{
                      input: "font-mono text-sm",
                    }}
                    placeholder="暂无数据"
                  />
                </div>
              )}
            </div>
          </ModalBody>
          <ModalFooter>
            <Button variant="light" onPress={() => setExportModalOpen(false)}>
              关闭
            </Button>
          </ModalFooter>
        </ModalContent>
      </Modal>

      {/* 导入数据模态框 */}
      <Modal
        isOpen={importModalOpen}
        onClose={() => setImportModalOpen(false)}
        size="2xl"
        scrollBehavior="outside"
        backdrop="blur"
        placement="center"
      >
        <ModalContent>
          <ModalHeader className="flex flex-col gap-1">
            <h2 className="text-xl font-bold">导入转发数据</h2>
            <p className="text-small text-default-500">
              格式：目标地址|转发名称|入口端口，每行一个，入口端口留空将自动分配可用端口
            </p>
            <p className="text-small text-default-400">
              目标地址支持单个地址(如：example.com:8080)或多个地址用逗号分隔(如：3.3.3.3:3,4.4.4.4:4)
            </p>
          </ModalHeader>
          <ModalBody className="pb-6">
            <div className="space-y-4">
              {/* 隧道选择 */}
              <div>
                <Select
                  isRequired
                    label="选择导入隧道"
                    placeholder="请选择要导入的隧道"
                    selectedKeys={selectedTunnelForImport ? [selectedTunnelForImport.toString()] : []}
                    variant="bordered"
                    onSelectionChange={(keys) => {
                      const selectedKey = Array.from(keys)[0] as string;
                      setSelectedTunnelForImport(selectedKey ? parseInt(selectedKey) : null);
                    }}
                >
                  {tunnels.map((tunnel) => (
                    <SelectItem
                      key={tunnel.id.toString()}
                      textValue={tunnel.name}
                    >
                      {tunnel.name}
                    </SelectItem>
                  ))}
                </Select>
              </div>

              {/* 输入区域 */}
              <div>
                <Textarea
                  classNames={{
                      input: "font-mono text-sm"
                    }}
                    label="导入数据"
                    maxRows={12}
                    minRows={8}
                    placeholder="请输入要导入的转发数据，格式：目标地址|转发名称|入口端口"
                    value={importData}
                    variant="flat"
                    onChange={(e) => setImportData(e.target.value)}
                />
              </div>

              {/* 导入结果 */}
              {importResults.length > 0 && (
                <div>
                  <div className="flex items-center justify-between mb-2">
                    <h3 className="text-base font-semibold">导入结果</h3>
                    <div className="flex items-center gap-2">
                      <span className="text-xs text-default-500">
                        成功：{importResults.filter((r) => r.success).length} /
                        总计：{importResults.length}
                      </span>
                    </div>
                  </div>

                  <div
                    className="max-h-40 overflow-y-auto space-y-1"
                    style={{
                      scrollbarWidth: "thin",
                      scrollbarColor: "rgb(156 163 175) transparent",
                    }}
                  >
                    {importResults.map((result, index) => (
                      <div
                        key={index}
                        className={`p-2 rounded border ${
                          result.success
                            ? "bg-success-50 dark:bg-success-100/10 border-success-200 dark:border-success-300/20"
                            : "bg-danger-50 dark:bg-danger-100/10 border-danger-200 dark:border-danger-300/20"
                        }`}
                      >
                        <div className="flex items-center gap-2">
                          {result.success ? (
                            <svg
                              className="w-3 h-3 text-success-600 flex-shrink-0"
                              fill="currentColor"
                              viewBox="0 0 20 20"
                            >
                              <path
                                fillRule="evenodd"
                                d="M16.707 5.293a1 1 0 010 1.414l-8 8a1 1 0 01-1.414 0l-4-4a1 1 0 011.414-1.414L8 12.586l7.293-7.293a1 1 0 011.414 0z"
                                clipRule="evenodd"
                              />
                            </svg>
                          ) : (
                            <svg
                              className="w-3 h-3 text-danger-600 flex-shrink-0"
                              fill="currentColor"
                              viewBox="0 0 20 20"
                            >
                              <path
                                fillRule="evenodd"
                                d="M4.293 4.293a1 1 0 011.414 0L10 8.586l4.293-4.293a1 1 0 111.414 1.414L11.414 10l4.293 4.293a1 1 0 01-1.414 1.414L10 11.414l-4.293 4.293a1 1 0 01-1.414-1.414L8.586 10 4.293 5.707a1 1 0 010-1.414z"
                                clipRule="evenodd"
                              />
                            </svg>
                          )}
                          <div className="flex-1 min-w-0">
                            <div className="flex items-center gap-2 mb-0.5">
                              <span
                                className={`text-xs font-medium ${
                                  result.success
                                    ? "text-success-700 dark:text-success-300"
                                    : "text-danger-700 dark:text-danger-300"
                                }`}
                              >
                                {result.success ? "成功" : "失败"}
                              </span>
                              <span className="text-xs text-default-500">
                                |
                              </span>
                              <code className="text-xs font-mono text-default-600 truncate">
                                {result.line}
                              </code>
                            </div>
                            <div
                              className={`text-xs ${
                                result.success
                                  ? "text-success-600 dark:text-success-400"
                                  : "text-danger-600 dark:text-danger-400"
                              }`}
                            >
                              {result.message}
                            </div>
                          </div>
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              )}
            </div>
          </ModalBody>
          <ModalFooter>
            <Button variant="light" onPress={() => setImportModalOpen(false)}>
              关闭
            </Button>
            <Button
              color="warning"
              onPress={executeImport}
              isLoading={importLoading}
              isDisabled={!importData.trim() || !selectedTunnelForImport}
            >
              开始导入
            </Button>
          </ModalFooter>
        </ModalContent>
      </Modal>

      {/* 诊断结果模态框 */}
      <Modal
        backdrop="blur"
          classNames={{
            base: "rounded-2xl",
            header: "rounded-t-2xl",
            body: "rounded-none",
            footer: "rounded-b-2xl"
          }}
          isOpen={diagnosisModalOpen}
          placement="center"
          scrollBehavior="inside"
          size="4xl"
          onOpenChange={setDiagnosisModalOpen}
      >
        <ModalContent>
          {(onClose) => (
            <>
              <ModalHeader className="flex flex-col gap-1 bg-content1 border-b border-divider">
                <h2 className="text-xl font-bold">转发诊断结果</h2>
                {currentDiagnosisForward && (
                  <div className="flex items-center gap-2 min-w-0">
                    <span className="text-small text-default-500 truncate flex-1 min-w-0">
                      {currentDiagnosisForward.name}
                    </span>
                    <Chip
                      color="primary"
                      variant="flat"
                      size="sm"
                      className="flex-shrink-0"
                    >
                      转发服务
                    </Chip>
                  </div>
                )}
              </ModalHeader>
              <ModalBody className="bg-content1">
                {diagnosisLoading ? (
                  <div className="flex items-center justify-center py-16">
                    <div className="flex items-center gap-3">
                      <Spinner size="sm" />
                      <span className="text-default-600">正在诊断...</span>
                    </div>
                  </div>
                ) : diagnosisResult ? (
                  <div className="space-y-4">
                    {/* 统计摘要 */}
                    <div className="grid grid-cols-3 gap-3">
                      <div className="text-center p-3 bg-default-100 dark:bg-gray-800 rounded-lg border border-divider">
                        <div className="text-2xl font-bold text-foreground">
                          {diagnosisResult.results.length}
                        </div>
                        <div className="text-xs text-default-500 mt-1">
                          总测试数
                        </div>
                      </div>
                      <div className="text-center p-3 bg-success-50 dark:bg-success-900/20 rounded-lg border border-success-200 dark:border-success-700">
                        <div className="text-2xl font-bold text-success-600 dark:text-success-400">
                          {
                            diagnosisResult.results.filter((r) => r.success)
                              .length
                          }
                        </div>
                        <div className="text-xs text-success-600 dark:text-success-400/80 mt-1">
                          成功
                        </div>
                      </div>
                      <div className="text-center p-3 bg-danger-50 dark:bg-danger-900/20 rounded-lg border border-danger-200 dark:border-danger-700">
                        <div className="text-2xl font-bold text-danger-600 dark:text-danger-400">
                          {
                            diagnosisResult.results.filter((r) => !r.success)
                              .length
                          }
                        </div>
                        <div className="text-xs text-danger-600 dark:text-danger-400/80 mt-1">
                          失败
                        </div>
                      </div>
                    </div>

                    {/* 桌面端表格展示 */}
                    <div className="hidden md:block space-y-3">
                      {(() => {
                        // 使用后端返回的 chainType 和 inx 字段进行分组
                        const groupedResults = {
                          entry: diagnosisResult.results.filter(
                            (r) => r.fromChainType === 1,
                          ),
                          chains: {} as Record<
                            number,
                            typeof diagnosisResult.results
                          >,
                          exit: diagnosisResult.results.filter(
                            (r) => r.fromChainType === 3,
                          ),
                        };

                        // 按 inx 分组链路测试
                        diagnosisResult.results.forEach((r) => {
                          if (r.fromChainType === 2 && r.fromInx != null) {
                            if (!groupedResults.chains[r.fromInx]) {
                              groupedResults.chains[r.fromInx] = [];
                            }
                            groupedResults.chains[r.fromInx].push(r);
                          }
                        });

                        const renderTableSection = (
                          title: string,
                          results: typeof diagnosisResult.results,
                        ) => {
                          if (results.length === 0) return null;

                          return (
                            <div
                              key={title}
                              className="border border-divider rounded-lg overflow-hidden bg-white dark:bg-gray-800"
                            >
                              <div className="bg-primary/10 dark:bg-primary/20 px-3 py-2 border-b border-divider">
                                <h3 className="text-sm font-semibold text-primary">
                                  {title}
                                </h3>
                              </div>
                              <table className="w-full text-sm">
                                <thead className="bg-default-100 dark:bg-gray-700">
                                  <tr>
                                    <th className="px-3 py-2 text-left font-semibold text-xs">
                                      路径
                                    </th>
                                    <th className="px-3 py-2 text-center font-semibold text-xs w-20">
                                      状态
                                    </th>
                                    <th className="px-3 py-2 text-center font-semibold text-xs w-24">
                                      延迟(ms)
                                    </th>
                                    <th className="px-3 py-2 text-center font-semibold text-xs w-24">
                                      丢包率
                                    </th>
                                    <th className="px-3 py-2 text-center font-semibold text-xs w-20">
                                      质量
                                    </th>
                                  </tr>
                                </thead>
                                <tbody className="divide-y divide-divider bg-white dark:bg-gray-800">
                                  {results.map((result, index) => {
                                    const quality = getQualityDisplay(
                                      result.averageTime,
                                      result.packetLoss,
                                    );

                                    return (
                                      <tr
                                        key={index}
                                        className={`hover:bg-default-50 dark:hover:bg-gray-700/50 ${
                                          result.success
                                            ? "bg-white dark:bg-gray-800"
                                            : "bg-danger-50 dark:bg-danger-900/30"
                                        }`}
                                      >
                                        <td className="px-3 py-2">
                                          <div className="flex items-center gap-2">
                                            <span
                                              className={`w-5 h-5 rounded-full flex items-center justify-center text-xs ${
                                                result.success
                                                  ? "bg-success text-white"
                                                  : "bg-danger text-white"
                                              }`}
                                            >
                                              {result.success ? "✓" : "✗"}
                                            </span>
                                            <div className="flex-1 min-w-0">
                                              <div className="font-medium text-foreground truncate">
                                                {result.description}
                                              </div>
                                              <div className="text-xs text-default-500 truncate">
                                                {result.targetIp}:
                                                {result.targetPort}
                                              </div>
                                            </div>
                                          </div>
                                        </td>
                                        <td className="px-3 py-2 text-center">
                                          <Chip
                                            color={
                                              result.success
                                                ? "success"
                                                : "danger"
                                            }
                                            variant="flat"
                                            size="sm"
                                            className="min-w-[50px]"
                                          >
                                            {result.success ? "成功" : "失败"}
                                          </Chip>
                                        </td>
                                        <td className="px-3 py-2 text-center">
                                          {result.success ? (
                                            <span className="font-semibold text-primary">
                                              {result.averageTime?.toFixed(0)}
                                            </span>
                                          ) : (
                                            <span className="text-default-400">
                                              -
                                            </span>
                                          )}
                                        </td>
                                        <td className="px-3 py-2 text-center">
                                          {result.success ? (
                                            <span
                                              className={`font-semibold ${
                                                (result.packetLoss || 0) > 0
                                                  ? "text-warning"
                                                  : "text-success"
                                              }`}
                                            >
                                              {result.packetLoss?.toFixed(1)}%
                                            </span>
                                          ) : (
                                            <span className="text-default-400">
                                              -
                                            </span>
                                          )}
                                        </td>
                                        <td className="px-3 py-2 text-center">
                                          {result.success && quality ? (
                                            <Chip
                                              color={quality.color as any}
                                              variant="flat"
                                              size="sm"
                                              className="text-xs"
                                            >
                                              {quality.text}
                                            </Chip>
                                          ) : (
                                            <span className="text-default-400">
                                              -
                                            </span>
                                          )}
                                        </td>
                                      </tr>
                                    );
                                  })}
                                </tbody>
                              </table>
                            </div>
                          );
                        };

                        return (
                          <>
                            {/* 入口测试 */}
                            {renderTableSection(
                              "🚪 入口测试",
                              groupedResults.entry,
                            )}

                            {/* 链路测试（按跳数排序） */}
                            {Object.keys(groupedResults.chains)
                              .map(Number)
                              .sort((a, b) => a - b)
                              .map((hop) =>
                                renderTableSection(
                                  `🔗 转发链 - 第${hop}跳`,
                                  groupedResults.chains[hop],
                                ),
                              )}

                            {/* 出口测试 */}
                            {renderTableSection(
                              "🚀 出口测试",
                              groupedResults.exit,
                            )}
                          </>
                        );
                      })()}
                    </div>

                    {/* 移动端卡片展示 */}
                    <div className="md:hidden space-y-3">
                      {(() => {
                        // 使用后端返回的 chainType 和 inx 字段进行分组
                        const groupedResults = {
                          entry: diagnosisResult.results.filter(
                            (r) => r.fromChainType === 1,
                          ),
                          chains: {} as Record<
                            number,
                            typeof diagnosisResult.results
                          >,
                          exit: diagnosisResult.results.filter(
                            (r) => r.fromChainType === 3,
                          ),
                        };

                        // 按 inx 分组链路测试
                        diagnosisResult.results.forEach((r) => {
                          if (r.fromChainType === 2 && r.fromInx != null) {
                            if (!groupedResults.chains[r.fromInx]) {
                              groupedResults.chains[r.fromInx] = [];
                            }
                            groupedResults.chains[r.fromInx].push(r);
                          }
                        });

                        const renderCardSection = (
                          title: string,
                          results: typeof diagnosisResult.results,
                        ) => {
                          if (results.length === 0) return null;

                          return (
                            <div key={title} className="space-y-2">
                              <div className="px-2 py-1.5 bg-primary/10 dark:bg-primary/20 rounded-lg border border-primary/30">
                                <h3 className="text-sm font-semibold text-primary">
                                  {title}
                                </h3>
                              </div>
                              {results.map((result, index) => {
                                const quality = getQualityDisplay(
                                  result.averageTime,
                                  result.packetLoss,
                                );

                                return (
                                  <div
                                    key={index}
                                    className={`border rounded-lg p-3 ${
                                      result.success
                                        ? "border-divider bg-white dark:bg-gray-800"
                                        : "border-danger-200 dark:border-danger-300/30 bg-danger-50 dark:bg-danger-900/30"
                                    }`}
                                  >
                                    <div className="flex items-start gap-2 mb-2">
                                      <span
                                        className={`w-6 h-6 rounded-full flex items-center justify-center text-xs flex-shrink-0 ${
                                          result.success
                                            ? "bg-success text-white"
                                            : "bg-danger text-white"
                                        }`}
                                      >
                                        {result.success ? "✓" : "✗"}
                                      </span>
                                      <div className="flex-1 min-w-0">
                                        <div className="font-semibold text-sm text-foreground break-words">
                                          {result.description}
                                        </div>
                                        <div className="text-xs text-default-500 mt-0.5 break-all">
                                          {result.targetIp}:{result.targetPort}
                                        </div>
                                      </div>
                                      <Chip
                                        color={
                                          result.success ? "success" : "danger"
                                        }
                                        variant="flat"
                                        size="sm"
                                        className="flex-shrink-0"
                                      >
                                        {result.success ? "成功" : "失败"}
                                      </Chip>
                                    </div>

                                    {result.success ? (
                                      <div className="grid grid-cols-3 gap-2 mt-2 pt-2 border-t border-divider">
                                        <div className="text-center">
                                          <div className="text-lg font-bold text-primary">
                                            {result.averageTime?.toFixed(0)}
                                          </div>
                                          <div className="text-xs text-default-500">
                                            延迟(ms)
                                          </div>
                                        </div>
                                        <div className="text-center">
                                          <div
                                            className={`text-lg font-bold ${
                                              (result.packetLoss || 0) > 0
                                                ? "text-warning"
                                                : "text-success"
                                            }`}
                                          >
                                            {result.packetLoss?.toFixed(1)}%
                                          </div>
                                          <div className="text-xs text-default-500">
                                            丢包率
                                          </div>
                                        </div>
                                        <div className="text-center">
                                          {quality && (
                                            <>
                                              <Chip
                                                color={quality.color as any}
                                                variant="flat"
                                                size="sm"
                                                className="text-xs"
                                              >
                                                {quality.text}
                                              </Chip>
                                              <div className="text-xs text-default-500 mt-0.5">
                                                质量
                                              </div>
                                            </>
                                          )}
                                        </div>
                                      </div>
                                    ) : (
                                      <div className="mt-2 pt-2 border-t border-divider">
                                        <div className="text-xs text-danger">
                                          {result.message || "连接失败"}
                                        </div>
                                      </div>
                                    )}
                                  </div>
                                );
                              })}
                            </div>
                          );
                        };

                        return (
                          <>
                            {/* 入口测试 */}
                            {renderCardSection(
                              "🚪 入口测试",
                              groupedResults.entry,
                            )}

                            {/* 链路测试（按跳数排序） */}
                            {Object.keys(groupedResults.chains)
                              .map(Number)
                              .sort((a, b) => a - b)
                              .map((hop) =>
                                renderCardSection(
                                  `🔗 转发链 - 第${hop}跳`,
                                  groupedResults.chains[hop],
                                ),
                              )}

                            {/* 出口测试 */}
                            {renderCardSection(
                              "🚀 出口测试",
                              groupedResults.exit,
                            )}
                          </>
                        );
                      })()}
                    </div>

                    {/* 失败详情（仅桌面端显示，移动端已在卡片中显示） */}
                    {diagnosisResult.results.some((r) => !r.success) && (
                      <div className="space-y-2 hidden md:block">
                        <h4 className="text-sm font-semibold text-danger">
                          失败详情
                        </h4>
                        <div className="space-y-2">
                          {diagnosisResult.results
                            .filter((r) => !r.success)
                            .map((result, index) => (
                              <Alert
                                key={index}
                                className="text-xs"
                                color="danger"
                                description={result.message || '连接失败'}
                                title={result.description}
                                variant="flat"
                              />
                            ))}
                        </div>
                      </div>
                    )}
                  </div>
                ) : (
                  <div className="text-center py-16">
                    <div className="w-16 h-16 bg-default-100 rounded-full flex items-center justify-center mx-auto mb-4">
                      <svg
                        className="w-8 h-8 text-default-400"
                        fill="none"
                        stroke="currentColor"
                        viewBox="0 0 24 24"
                      >
                        <path
                          strokeLinecap="round"
                          strokeLinejoin="round"
                          strokeWidth={1.5}
                          d="M9.75 9.75l4.5 4.5m0-4.5l-4.5 4.5M21 12a9 9 0 11-18 0 9 9 0 0118 0z"
                        />
                      </svg>
                    </div>
                    <h3 className="text-lg font-semibold text-foreground">
                      暂无诊断数据
                    </h3>
                  </div>
                )}
              </ModalBody>
              <ModalFooter className="bg-content1 border-t border-divider">
                <Button variant="light" onPress={onClose}>
                  关闭
                </Button>
                {currentDiagnosisForward && (
                  <Button
                    color="primary"
                    onPress={() => handleDiagnose(currentDiagnosisForward)}
                    isLoading={diagnosisLoading}
                  >
                    重新诊断
                  </Button>
                )}
              </ModalFooter>
            </>
          )}
        </ModalContent>
      </Modal>

      {/* 批量更改隧道模态框 */}
      <Modal
        isOpen={batchModalOpen}
          placement="center"
          size="sm"
          onOpenChange={setBatchModalOpen}
      >
        <ModalContent>
          {(onClose) => (
            <>
              <ModalHeader className="flex flex-col gap-1">
                <h2 className="text-xl font-bold">批量更改隧道</h2>
                <p className="text-small text-default-500">
                  将选中的 {selectedIds.size} 个转发迁移到目标隧道
                </p>
              </ModalHeader>
              <ModalBody>
                <Select
                  label="目标隧道"
                    placeholder="请选择新的隧道"
                    selectedKeys={batchTargetTunnelId !== null ? [batchTargetTunnelId.toString()] : []}
                    variant="bordered"
                    onSelectionChange={(keys) => {
                      const key = Array.from(keys)[0] as string | undefined;
                      setBatchTargetTunnelId(key ? parseInt(key) : null);
                    }}
                >
                  {tunnels.map((tunnel) => (
                    <SelectItem key={tunnel.id}>{tunnel.name}</SelectItem>
                  ))}
                </Select>
                <p className="text-xs text-default-400 mt-2">
                  批量迁移将逐个处理每个转发，部分失败不影响其他转发。
                </p>
              </ModalBody>
              <ModalFooter>
                <Button variant="light" onPress={onClose}>
                  取消
                </Button>
                <Button
                  color="primary"
                  onPress={confirmBatchChangeTunnel}
                  isLoading={batchLoading}
                >
                  确认迁移
                </Button>
              </ModalFooter>
            </>
          )}
        </ModalContent>
      </Modal>
    </div>
  );
}
