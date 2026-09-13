import { useState, useEffect } from "react";
import { Button } from "@heroui/button";
import { Input } from "@heroui/input";
import {
  Modal,
  ModalContent,
  ModalHeader,
  ModalBody,
  ModalFooter,
  useDisclosure,
} from "@heroui/modal";
import { Chip } from "@heroui/chip";
import { Select, SelectItem } from "@heroui/select";
import { RadioGroup, Radio } from "@heroui/radio";
import { DatePicker } from "@heroui/date-picker";
import { Spinner } from "@heroui/spinner";
import { Progress } from "@heroui/progress";
import { Divider } from "@heroui/divider";
import toast from "react-hot-toast";
import { parseDate } from "@internationalized/date";

import {
  User,
  UserForm,
  UserTunnel,
  UserTunnelForm,
  Tunnel,
  SpeedLimit,
  Pagination as PaginationType,
} from "@/types";
import {
  getAllUsers,
  createUser,
  updateUser,
  deleteUser,
  getTunnelList,
  assignUserTunnel,
  getUserTunnelList,
  removeUserTunnel,
  updateUserTunnel,
  getSpeedLimitList,
  resetUserFlow,
} from "@/api";
import {
  SearchIcon,
  EditIcon,
  DeleteIcon,
  UserIcon,
  SettingsIcon,
} from "@/components/icons";

// 工具函数
const formatFlow = (value: number, unit: string = "bytes"): string => {
  if (unit === "gb") {
    return `${value} GB`;
  } else {
    if (value === 0) return "0 B";
    if (value < 1024) return `${value} B`;
    if (value < 1024 * 1024) return `${(value / 1024).toFixed(2)} KB`;
    if (value < 1024 * 1024 * 1024)
      return `${(value / (1024 * 1024)).toFixed(2)} MB`;

    return `${(value / (1024 * 1024 * 1024)).toFixed(2)} GB`;
  }
};

const formatDate = (timestamp: number): string => {
  return new Date(timestamp).toLocaleString();
};

const getExpireStatus = (expTime: number) => {
  const now = Date.now();

  if (expTime < now) {
    return { color: "danger" as const, text: "已过期" };
  }
  const diffDays = Math.ceil((expTime - now) / (1000 * 60 * 60 * 24));

  if (diffDays <= 7) {
    return { color: "warning" as const, text: `${diffDays}天后过期` };
  }

  return { color: "success" as const, text: "正常" };
};

// 获取用户状态（根据status字段）
const getUserStatus = (user: User) => {
  if (user.status === 1) {
    return { color: "success" as const, text: "正常" };
  } else {
    return { color: "danger" as const, text: "禁用" };
  }
};

const calculateUserTotalUsedFlow = (user: User): number => {
  return (user.inFlow || 0) + (user.outFlow || 0);
};

const calculateTunnelUsedFlow = (tunnel: UserTunnel): number => {
  const inFlow = tunnel.inFlow || 0;
  const outFlow = tunnel.outFlow || 0;

  // 后端已按计费类型处理流量，前端直接使用入站+出站总和
  return inFlow + outFlow;
};

export default function UserPage() {
  // 状态管理
  const [users, setUsers] = useState<User[]>([]);
  const [loading, setLoading] = useState(false);
  const [searchKeyword, setSearchKeyword] = useState("");
  const [pagination, setPagination] = useState<PaginationType>({
    current: 1,
    size: 10,
    total: 0,
  });

  // 用户表单相关状态
  const {
    isOpen: isUserModalOpen,
    onOpen: onUserModalOpen,
    onClose: onUserModalClose,
  } = useDisclosure();
  const [isEdit, setIsEdit] = useState(false);
  const [userForm, setUserForm] = useState<UserForm>({
    user: "",
    pwd: "",
    status: 1,
    flow: 100,
    num: 10,
    expTime: null,
    flowResetTime: 0,
  });
  const [userFormLoading, setUserFormLoading] = useState(false);

  // 隧道权限管理相关状态
  const {
    isOpen: isTunnelModalOpen,
    onOpen: onTunnelModalOpen,
    onClose: onTunnelModalClose,
  } = useDisclosure();
  const [currentUser, setCurrentUser] = useState<User | null>(null);
  const [userTunnels, setUserTunnels] = useState<UserTunnel[]>([]);
  const [tunnelListLoading, setTunnelListLoading] = useState(false);

  // 分配新隧道权限相关状态
  const [tunnelForm, setTunnelForm] = useState<UserTunnelForm>({
    tunnelId: null,
    flow: 100,
    num: 10,
    expTime: null,
    flowResetTime: 0,
    speedId: null,
  });
  const [assignLoading, setAssignLoading] = useState(false);

  // 编辑隧道权限相关状态
  const {
    isOpen: isEditTunnelModalOpen,
    onOpen: onEditTunnelModalOpen,
    onClose: onEditTunnelModalClose,
  } = useDisclosure();
  const [editTunnelForm, setEditTunnelForm] = useState<UserTunnel | null>(null);
  const [editTunnelLoading, setEditTunnelLoading] = useState(false);

  // 删除确认相关状态
  const {
    isOpen: isDeleteModalOpen,
    onOpen: onDeleteModalOpen,
    onClose: onDeleteModalClose,
  } = useDisclosure();
  const [userToDelete, setUserToDelete] = useState<User | null>(null);

  // 删除隧道权限确认相关状态
  const {
    isOpen: isDeleteTunnelModalOpen,
    onOpen: onDeleteTunnelModalOpen,
    onClose: onDeleteTunnelModalClose,
  } = useDisclosure();
  const [tunnelToDelete, setTunnelToDelete] = useState<UserTunnel | null>(null);

  // 重置流量确认相关状态
  const {
    isOpen: isResetFlowModalOpen,
    onOpen: onResetFlowModalOpen,
    onClose: onResetFlowModalClose,
  } = useDisclosure();
  const [userToReset, setUserToReset] = useState<User | null>(null);
  const [resetFlowLoading, setResetFlowLoading] = useState(false);

  // 重置隧道流量确认相关状态
  const {
    isOpen: isResetTunnelFlowModalOpen,
    onOpen: onResetTunnelFlowModalOpen,
    onClose: onResetTunnelFlowModalClose,
  } = useDisclosure();
  const [tunnelToReset, setTunnelToReset] = useState<UserTunnel | null>(null);
  const [resetTunnelFlowLoading, setResetTunnelFlowLoading] = useState(false);

  // 其他数据
  const [tunnels, setTunnels] = useState<Tunnel[]>([]);
  const [speedLimits, setSpeedLimits] = useState<SpeedLimit[]>([]);

  // 生命周期
  useEffect(() => {
    loadUsers();
    loadTunnels();
    loadSpeedLimits();
  }, [pagination.current, pagination.size, searchKeyword]);

  // 数据加载函数
  const loadUsers = async () => {
    setLoading(true);
    try {
      const response = await getAllUsers({
        current: pagination.current,
        size: pagination.size,
        keyword: searchKeyword,
      });

      if (response.code === 0) {
        const data = response.data || {};

        setUsers(data || []);
      } else {
        toast.error(response.msg || "获取用户列表失败");
      }
    } catch (error) {
      toast.error("获取用户列表失败");
    } finally {
      setLoading(false);
    }
  };

  const loadTunnels = async () => {
    try {
      const response = await getTunnelList();

      if (response.code === 0) {
        setTunnels(response.data || []);
      }
    } catch (error) {
      console.error("获取隧道列表失败:", error);
    }
  };

  const loadSpeedLimits = async () => {
    try {
      const response = await getSpeedLimitList();

      if (response.code === 0) {
        setSpeedLimits(response.data || []);
      }
    } catch (error) {
      console.error("获取限速规则列表失败:", error);
    }
  };

  const loadUserTunnels = async (userId: number) => {
    setTunnelListLoading(true);
    try {
      const response = await getUserTunnelList({ userId });

      if (response.code === 0) {
        setUserTunnels(response.data || []);
      } else {
        toast.error(response.msg || "获取隧道权限列表失败");
      }
    } catch (error) {
      toast.error("获取隧道权限列表失败");
    } finally {
      setTunnelListLoading(false);
    }
  };

  // 用户管理操作
  const handleSearch = () => {
    setPagination((prev) => ({ ...prev, current: 1 }));
    loadUsers();
  };

  const handleAdd = () => {
    setIsEdit(false);
    setUserForm({
      user: "",
      pwd: "",
      status: 1,
      flow: 100,
      num: 10,
      expTime: null,
      flowResetTime: 0,
    });
    onUserModalOpen();
  };

  const handleEdit = (user: User) => {
    setIsEdit(true);
    setUserForm({
      id: user.id,
      name: user.name,
      user: user.user,
      pwd: "",
      status: user.status,
      flow: user.flow,
      num: user.num,
      expTime: user.expTime ? new Date(user.expTime) : null,
      flowResetTime: user.flowResetTime ?? 0,
    });
    onUserModalOpen();
  };

  const handleDelete = (user: User) => {
    setUserToDelete(user);
    onDeleteModalOpen();
  };

  const handleConfirmDelete = async () => {
    if (!userToDelete) return;

    try {
      const response = await deleteUser(userToDelete.id);

      if (response.code === 0) {
        toast.success("删除成功");
        loadUsers();
        onDeleteModalClose();
        setUserToDelete(null);
      } else {
        toast.error(response.msg || "删除失败");
      }
    } catch (error) {
      toast.error("删除失败");
    }
  };

  const handleSubmitUser = async () => {
    if (!userForm.user || (!userForm.pwd && !isEdit) || !userForm.expTime) {
      toast.error("请填写完整信息");

      return;
    }

    setUserFormLoading(true);
    try {
      const submitData: any = {
        ...userForm,
        expTime: userForm.expTime.getTime(),
      };

      if (isEdit && !submitData.pwd) {
        delete submitData.pwd;
      }

      const response = isEdit
        ? await updateUser(submitData)
        : await createUser(submitData);

      if (response.code === 0) {
        toast.success(isEdit ? "更新成功" : "创建成功");
        onUserModalClose();
        loadUsers();
      } else {
        toast.error(response.msg || (isEdit ? "更新失败" : "创建失败"));
      }
    } catch (error) {
      toast.error(isEdit ? "更新失败" : "创建失败");
    } finally {
      setUserFormLoading(false);
    }
  };

  // 隧道权限管理操作
  const handleManageTunnels = (user: User) => {
    setCurrentUser(user);
    setTunnelForm({
      tunnelId: null,
      flow: 100,
      num: 10,
      expTime: null,
      flowResetTime: 0,
      speedId: null,
    });
    onTunnelModalOpen();
    loadUserTunnels(user.id);
  };

  const handleAssignTunnel = async () => {
    if (!tunnelForm.tunnelId || !tunnelForm.expTime || !currentUser) {
      toast.error("请填写完整信息");

      return;
    }

    setAssignLoading(true);
    try {
      const response = await assignUserTunnel({
        userId: currentUser.id,
        tunnelId: tunnelForm.tunnelId,
        flow: tunnelForm.flow,
        num: tunnelForm.num,
        expTime: tunnelForm.expTime.getTime(),
        flowResetTime: tunnelForm.flowResetTime,
        speedId: tunnelForm.speedId,
      });

      if (response.code === 0) {
        toast.success("分配成功");
        setTunnelForm({
          tunnelId: null,
          flow: 100,
          num: 10,
          expTime: null,
          flowResetTime: 0,
          speedId: null,
        });
        loadUserTunnels(currentUser.id);
      } else {
        toast.error(response.msg || "分配失败");
      }
    } catch (error) {
      toast.error("分配失败");
    } finally {
      setAssignLoading(false);
    }
  };

  const handleEditTunnel = (userTunnel: UserTunnel) => {
    setEditTunnelForm({
      ...userTunnel,
      expTime: userTunnel.expTime,
    });
    onEditTunnelModalOpen();
  };

  const handleUpdateTunnel = async () => {
    if (!editTunnelForm) return;

    setEditTunnelLoading(true);
    try {
      const response = await updateUserTunnel({
        id: editTunnelForm.id,
        flow: editTunnelForm.flow,
        num: editTunnelForm.num,
        expTime: editTunnelForm.expTime,
        flowResetTime: editTunnelForm.flowResetTime,
        speedId: editTunnelForm.speedId,
        status: editTunnelForm.status,
      });

      if (response.code === 0) {
        toast.success("更新成功");
        onEditTunnelModalClose();
        if (currentUser) {
          loadUserTunnels(currentUser.id);
        }
      } else {
        toast.error(response.msg || "更新失败");
      }
    } catch (error) {
      toast.error("更新失败");
    } finally {
      setEditTunnelLoading(false);
    }
  };

  const handleRemoveTunnel = (userTunnel: UserTunnel) => {
    setTunnelToDelete(userTunnel);
    onDeleteTunnelModalOpen();
  };

  const handleConfirmRemoveTunnel = async () => {
    if (!tunnelToDelete) return;

    try {
      const response = await removeUserTunnel({ id: tunnelToDelete.id });

      if (response.code === 0) {
        toast.success("删除成功");
        if (currentUser) {
          loadUserTunnels(currentUser.id);
        }
        onDeleteTunnelModalClose();
        setTunnelToDelete(null);
      } else {
        toast.error(response.msg || "删除失败");
      }
    } catch (error) {
      toast.error("删除失败");
    }
  };

  // 重置流量相关函数
  const handleResetFlow = (user: User) => {
    setUserToReset(user);
    onResetFlowModalOpen();
  };

  const handleConfirmResetFlow = async () => {
    if (!userToReset) return;

    setResetFlowLoading(true);
    try {
      const response = await resetUserFlow({
        id: userToReset.id,
        type: 1, // 1表示重置用户流量
      });

      if (response.code === 0) {
        toast.success("流量重置成功");
        onResetFlowModalClose();
        setUserToReset(null);
        loadUsers(); // 重新加载用户列表
      } else {
        toast.error(response.msg || "重置失败");
      }
    } catch (error) {
      toast.error("重置失败");
    } finally {
      setResetFlowLoading(false);
    }
  };

  // 隧道流量重置相关函数
  const handleResetTunnelFlow = (userTunnel: UserTunnel) => {
    setTunnelToReset(userTunnel);
    onResetTunnelFlowModalOpen();
  };

  const handleConfirmResetTunnelFlow = async () => {
    if (!tunnelToReset) return;

    setResetTunnelFlowLoading(true);
    try {
      const response = await resetUserFlow({
        id: tunnelToReset.id,
        type: 2, // 2表示重置隧道流量
      });

      if (response.code === 0) {
        toast.success("隧道流量重置成功");
        onResetTunnelFlowModalClose();
        setTunnelToReset(null);
        if (currentUser) {
          loadUserTunnels(currentUser.id); // 重新加载隧道权限列表
        }
      } else {
        toast.error(response.msg || "重置失败");
      }
    } catch (error) {
      toast.error("重置失败");
    } finally {
      setResetTunnelFlowLoading(false);
    }
  };

  // 过滤数据
  const availableTunnels = tunnels.filter(
    (tunnel) => !userTunnels.some((ut) => ut.tunnelId === tunnel.id),
  );

  const availableSpeedLimits = speedLimits.filter(
    (speedLimit) => speedLimit.tunnelId === tunnelForm.tunnelId,
  );

  const editAvailableSpeedLimits = speedLimits.filter(
    (speedLimit) => speedLimit.tunnelId === editTunnelForm?.tunnelId,
  );

  const cellClass = "px-3 py-3 align-middle";
  const activeCount = users.filter((u) => u.status === 1).length;

  return (
    <div className="px-3 lg:px-6 py-8">
      {/* 页面头部 */}
      <div className="flex flex-col gap-4 mb-6">
        <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3">
          <div className="min-w-0">
            <h1 className="page-title">用户管理</h1>
            <p className="page-desc">
              共 {pagination.total || users.length} 个用户
              {users.length > 0 && (
                <span className="text-default-400">
                  {" "}
                  · 正常 {activeCount} · 禁用 {users.length - activeCount}
                </span>
              )}
            </p>
          </div>
          <Button color="primary" size="sm" variant="flat" onPress={handleAdd}>
            新增
          </Button>
        </div>

        <div className="flex flex-col sm:flex-row gap-3 items-stretch sm:items-center">
          <div className="flex items-center gap-3 flex-1 max-w-md">
            <Input
              className="flex-1"
              classNames={{
                inputWrapper: "bg-content1 border border-divider shadow-none",
              }}
              placeholder="搜索用户名"
              size="sm"
              startContent={<SearchIcon className="w-4 h-4 text-default-400" />}
              value={searchKeyword}
              onChange={(e) => setSearchKeyword(e.target.value)}
              onKeyDown={(e) => e.key === "Enter" && handleSearch()}
            />
            <Button
              isIconOnly
              aria-label="搜索"
              color="primary"
              size="sm"
              variant="flat"
              onPress={handleSearch}
            >
              <SearchIcon className="w-4 h-4" />
            </Button>
          </div>
        </div>
      </div>

      {/* 用户列表 */}
      {loading ? (
        <div className="flex items-center justify-center h-64">
          <div className="flex items-center gap-3">
            <Spinner size="sm" />
            <span className="text-default-500 text-sm">正在加载...</span>
          </div>
        </div>
      ) : users.length === 0 ? (
        <div className="text-center py-16 panel-card border border-divider shadow-panel bg-content1">
          <div className="flex flex-col items-center gap-4">
            <div className="empty-state-icon">
              <UserIcon className="w-8 h-8" />
            </div>
            <div>
              <h3 className="text-lg font-semibold text-foreground">
                暂无用户数据
              </h3>
              <p className="text-default-500 text-sm mt-1">
                还没有创建任何用户，点击上方按钮开始创建
              </p>
            </div>
          </div>
        </div>
      ) : (
        <div className="panel-table-wrap">
          <table aria-label="用户列表" className="w-full text-sm min-w-[960px]">
            <thead>
              <tr className="bg-default-50 text-default-600 font-medium text-xs">
                <th className="px-3 py-2.5 text-left">用户</th>
                <th className="px-3 py-2.5 text-left">状态</th>
                <th className="px-3 py-2.5 text-left min-w-[10rem]">流量</th>
                <th className="px-3 py-2.5 text-left">转发配额</th>
                <th className="px-3 py-2.5 text-left">重置</th>
                <th className="px-3 py-2.5 text-left">有效期</th>
                <th className="px-3 py-2.5 text-right">操作</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-divider">
              {users.map((user) => {
                const userStatus = getUserStatus(user);
                const expStatus = user.expTime
                  ? getExpireStatus(user.expTime)
                  : null;
                const usedFlow = calculateUserTotalUsedFlow(user);
                const flowLimitBytes = (user.flow || 0) * 1024 * 1024 * 1024;
                const isUnlimited = user.flow === 99999;
                const flowPercent =
                  !isUnlimited && flowLimitBytes > 0
                    ? Math.min((usedFlow / flowLimitBytes) * 100, 100)
                    : 0;

                return (
                  <tr
                    key={user.id}
                    className="bg-content1 hover:bg-default-50/70 transition-colors"
                  >
                    <td className={cellClass}>
                      <div className="min-w-0">
                        <div className="font-medium text-foreground text-sm truncate">
                          {user.name || user.user}
                        </div>
                        <div className="text-xs text-default-500 truncate">
                          @{user.user}
                        </div>
                      </div>
                    </td>
                    <td className={cellClass}>
                      <Chip
                        className="text-xs"
                        color={userStatus.color}
                        size="sm"
                        startContent={
                          <span
                            className={`inline-block w-1.5 h-1.5 rounded-full ml-1 ${user.status === 1 ? "bg-success" : "bg-danger"}`}
                          />
                        }
                        variant="flat"
                      >
                        {userStatus.text}
                      </Chip>
                    </td>
                    <td className={cellClass}>
                      <div className="min-w-[9rem] max-w-[14rem]">
                        <div className="flex justify-between text-xs mb-1 gap-2">
                          <span className="text-default-500 truncate">
                            {isUnlimited
                              ? "无限制"
                              : formatFlow(user.flow, "gb")}
                          </span>
                          <span className="font-mono text-danger whitespace-nowrap">
                            {formatFlow(usedFlow)}
                          </span>
                        </div>
                        <Progress
                          aria-label={`${user.user} 流量使用 ${flowPercent.toFixed(1)}%`}
                          color={
                            flowPercent > 90
                              ? "danger"
                              : flowPercent > 70
                                ? "warning"
                                : "success"
                          }
                          size="sm"
                          value={isUnlimited ? 0 : flowPercent}
                        />
                        <div className="text-[11px] text-default-400 mt-0.5">
                          {isUnlimited
                            ? "已用流量"
                            : `已用 ${flowPercent.toFixed(1)}%`}
                        </div>
                      </div>
                    </td>
                    <td className={cellClass}>
                      <span className="text-xs font-medium text-foreground">
                        {user.num === 99999 ? "无限制" : user.num}
                      </span>
                    </td>
                    <td className={cellClass}>
                      <span className="text-xs text-default-600 whitespace-nowrap">
                        {user.flowResetTime === 0
                          ? "不重置"
                          : `每月${user.flowResetTime}号`}
                      </span>
                    </td>
                    <td className={cellClass}>
                      {user.expTime ? (
                        <div className="space-y-1">
                          <div className="text-xs text-default-600 whitespace-nowrap">
                            {formatDate(user.expTime)}
                          </div>
                          {expStatus && expStatus.color !== "success" && (
                            <Chip
                              className="text-xs"
                              color={expStatus.color}
                              size="sm"
                              variant="flat"
                            >
                              {expStatus.text}
                            </Chip>
                          )}
                        </div>
                      ) : (
                        <span className="text-xs text-default-400">永久</span>
                      )}
                    </td>
                    <td className={cellClass}>
                      <div className="flex items-center gap-1 justify-end flex-wrap">
                        <Button
                          className="min-w-unit-14"
                          color="primary"
                          size="sm"
                          startContent={<EditIcon className="w-3 h-3" />}
                          variant="flat"
                          onPress={() => handleEdit(user)}
                        >
                          编辑
                        </Button>
                        <Button
                          className="min-w-unit-14"
                          color="warning"
                          size="sm"
                          variant="flat"
                          onPress={() => handleResetFlow(user)}
                        >
                          重置
                        </Button>
                        <Button
                          className="min-w-unit-14"
                          color="success"
                          size="sm"
                          startContent={<SettingsIcon className="w-3 h-3" />}
                          variant="flat"
                          onPress={() => handleManageTunnels(user)}
                        >
                          权限
                        </Button>
                        <Button
                          className="min-w-unit-14"
                          color="danger"
                          size="sm"
                          startContent={<DeleteIcon className="w-3 h-3" />}
                          variant="flat"
                          onPress={() => handleDelete(user)}
                        >
                          删除
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

      {/* 用户表单模态框 */}
      <Modal
        backdrop="blur"
        isOpen={isUserModalOpen}
        placement="center"
        scrollBehavior="outside"
        size="2xl"
        onClose={onUserModalClose}
      >
        <ModalContent>
          <ModalHeader>{isEdit ? "编辑用户" : "新增用户"}</ModalHeader>
          <ModalBody>
            <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
              <Input
                isRequired
                label="用户名"
                value={userForm.user}
                onChange={(e) =>
                  setUserForm((prev) => ({ ...prev, user: e.target.value }))
                }
              />
              <Input
                isRequired={!isEdit}
                label="密码"
                placeholder={isEdit ? "留空则不修改密码" : "请输入密码"}
                type="password"
                value={userForm.pwd}
                onChange={(e) =>
                  setUserForm((prev) => ({ ...prev, pwd: e.target.value }))
                }
              />
              <Input
                isRequired
                label="流量限制(GB)"
                max="99999"
                min="1"
                type="number"
                value={userForm.flow.toString()}
                onChange={(e) => {
                  const value = Math.min(
                    Math.max(Number(e.target.value) || 0, 1),
                    99999,
                  );

                  setUserForm((prev) => ({ ...prev, flow: value }));
                }}
              />
              <Input
                isRequired
                label="转发数量"
                max="99999"
                min="1"
                type="number"
                value={userForm.num.toString()}
                onChange={(e) => {
                  const value = Math.min(
                    Math.max(Number(e.target.value) || 0, 1),
                    99999,
                  );

                  setUserForm((prev) => ({ ...prev, num: value }));
                }}
              />
              <Select
                label="流量重置日期"
                selectedKeys={[userForm.flowResetTime.toString()]}
                onSelectionChange={(keys) => {
                  const value = Array.from(keys)[0] as string;

                  setUserForm((prev) => ({
                    ...prev,
                    flowResetTime: Number(value),
                  }));
                }}
              >
                <>
                  <SelectItem key="0" textValue="不重置">
                    不重置
                  </SelectItem>
                  {Array.from({ length: 31 }, (_, i) => i + 1).map((day) => (
                    <SelectItem
                      key={day.toString()}
                      textValue={`每月${day}号（0点重置）`}
                    >
                      每月{day}号（0点重置）
                    </SelectItem>
                  ))}
                </>
              </Select>
              <DatePicker
                isRequired
                showMonthAndYearPickers
                className="cursor-pointer"
                label="过期时间"
                value={
                  userForm.expTime
                    ? (parseDate(
                        userForm.expTime.toISOString().split("T")[0],
                      ) as any)
                    : null
                }
                onChange={(date) => {
                  if (date) {
                    const jsDate = new Date(
                      date.year,
                      date.month - 1,
                      date.day,
                      23,
                      59,
                      59,
                    );

                    setUserForm((prev) => ({ ...prev, expTime: jsDate }));
                  } else {
                    setUserForm((prev) => ({ ...prev, expTime: null }));
                  }
                }}
              />
            </div>

            <RadioGroup
              label="状态"
              orientation="horizontal"
              value={userForm.status.toString()}
              onValueChange={(value: string) =>
                setUserForm((prev) => ({ ...prev, status: Number(value) }))
              }
            >
              <Radio value="1">正常</Radio>
              <Radio value="0">禁用</Radio>
            </RadioGroup>
          </ModalBody>
          <ModalFooter>
            <Button onPress={onUserModalClose}>取消</Button>
            <Button
              color="primary"
              isLoading={userFormLoading}
              onPress={handleSubmitUser}
            >
              确定
            </Button>
          </ModalFooter>
        </ModalContent>
      </Modal>

      {/* 隧道权限管理模态框 */}
      <Modal
        backdrop="blur"
        classNames={{
          base: "max-w-[96vw] sm:max-w-5xl",
          body: "py-4",
        }}
        isDismissable={false}
        isOpen={isTunnelModalOpen}
        placement="center"
        scrollBehavior="inside"
        size="5xl"
        onClose={onTunnelModalClose}
      >
        <ModalContent>
          <ModalHeader className="flex flex-col items-start gap-1 pb-2">
            <div className="flex items-center gap-2 flex-wrap">
              <span className="text-lg font-semibold tracking-tight">
                隧道权限
              </span>
              {currentUser && (
                <Chip
                  className="text-xs"
                  color="primary"
                  size="sm"
                  variant="flat"
                >
                  @{currentUser.user}
                </Chip>
              )}
            </div>
            <p className="text-xs font-normal text-default-500">
              已分配 {userTunnels.length} 条
              {availableTunnels.length > 0
                ? ` · 还可分配 ${availableTunnels.length} 条`
                : tunnels.length > 0
                  ? " · 全部隧道已分配"
                  : " · 暂无可用隧道"}
            </p>
          </ModalHeader>
          <ModalBody>
            <div className="space-y-5">
              {/* 分配新权限 */}
              <section className="panel-surface-muted p-4 space-y-3">
                <div className="flex items-center justify-between gap-2">
                  <div>
                    <h3 className="text-sm font-semibold text-foreground">
                      分配新权限
                    </h3>
                    <p className="text-xs text-default-500 mt-0.5">
                      选择未分配隧道并设置配额、限速与有效期
                    </p>
                  </div>
                  {availableTunnels.length === 0 && (
                    <Chip
                      className="text-xs shrink-0"
                      color="default"
                      size="sm"
                      variant="flat"
                    >
                      无可分配
                    </Chip>
                  )}
                </div>

                <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-3 gap-3">
                  <Select
                    classNames={{ trigger: "bg-content1" }}
                    isDisabled={availableTunnels.length === 0}
                    label="隧道"
                    placeholder="选择隧道"
                    selectedKeys={
                      tunnelForm.tunnelId
                        ? [tunnelForm.tunnelId.toString()]
                        : []
                    }
                    size="sm"
                    onSelectionChange={(keys) => {
                      const value = Array.from(keys)[0] as string;

                      setTunnelForm((prev) => ({
                        ...prev,
                        tunnelId: Number(value) || null,
                        speedId: null,
                      }));
                    }}
                  >
                    {availableTunnels.map((tunnel) => (
                      <SelectItem
                        key={tunnel.id.toString()}
                        textValue={tunnel.name}
                      >
                        {tunnel.name}
                      </SelectItem>
                    ))}
                  </Select>

                  <Select
                    classNames={{ trigger: "bg-content1" }}
                    isDisabled={!tunnelForm.tunnelId}
                    label="限速规则"
                    selectedKeys={
                      tunnelForm.speedId
                        ? [tunnelForm.speedId.toString()]
                        : ["null"]
                    }
                    size="sm"
                    onSelectionChange={(keys) => {
                      const value = Array.from(keys)[0] as string;

                      setTunnelForm((prev) => ({
                        ...prev,
                        speedId: value === "null" ? null : Number(value),
                      }));
                    }}
                  >
                    {[
                      <SelectItem key="null" textValue="不限速">
                        不限速
                      </SelectItem>,
                      ...availableSpeedLimits.map((speedLimit) => (
                        <SelectItem
                          key={speedLimit.id.toString()}
                          textValue={speedLimit.name}
                        >
                          {speedLimit.name}
                        </SelectItem>
                      )),
                    ]}
                  </Select>

                  <Input
                    classNames={{ inputWrapper: "bg-content1" }}
                    label="流量限制 (GB)"
                    max="99999"
                    min="1"
                    size="sm"
                    type="number"
                    value={tunnelForm.flow.toString()}
                    onChange={(e) => {
                      const value = Math.min(
                        Math.max(Number(e.target.value) || 0, 1),
                        99999,
                      );

                      setTunnelForm((prev) => ({ ...prev, flow: value }));
                    }}
                  />

                  <Input
                    classNames={{ inputWrapper: "bg-content1" }}
                    label="转发数量"
                    max="99999"
                    min="1"
                    size="sm"
                    type="number"
                    value={tunnelForm.num.toString()}
                    onChange={(e) => {
                      const value = Math.min(
                        Math.max(Number(e.target.value) || 0, 1),
                        99999,
                      );

                      setTunnelForm((prev) => ({ ...prev, num: value }));
                    }}
                  />

                  <Select
                    classNames={{ trigger: "bg-content1" }}
                    label="流量重置"
                    selectedKeys={[tunnelForm.flowResetTime.toString()]}
                    size="sm"
                    onSelectionChange={(keys) => {
                      const value = Array.from(keys)[0] as string;

                      setTunnelForm((prev) => ({
                        ...prev,
                        flowResetTime: Number(value),
                      }));
                    }}
                  >
                    <>
                      <SelectItem key="0" textValue="不重置">
                        不重置
                      </SelectItem>
                      {Array.from({ length: 31 }, (_, i) => i + 1).map(
                        (day) => (
                          <SelectItem
                            key={day.toString()}
                            textValue={`每月${day}号（0点）`}
                          >
                            每月{day}号（0点）
                          </SelectItem>
                        ),
                      )}
                    </>
                  </Select>

                  <DatePicker
                    showMonthAndYearPickers
                    className="cursor-pointer"
                    classNames={{ inputWrapper: "bg-content1" }}
                    label="到期时间"
                    size="sm"
                    value={
                      tunnelForm.expTime
                        ? (parseDate(
                            tunnelForm.expTime.toISOString().split("T")[0],
                          ) as any)
                        : null
                    }
                    onChange={(date) => {
                      if (date) {
                        const jsDate = new Date(
                          date.year,
                          date.month - 1,
                          date.day,
                          23,
                          59,
                          59,
                        );

                        setTunnelForm((prev) => ({ ...prev, expTime: jsDate }));
                      } else {
                        setTunnelForm((prev) => ({ ...prev, expTime: null }));
                      }
                    }}
                  />
                </div>

                <div className="flex justify-end">
                  <Button
                    color="primary"
                    isDisabled={availableTunnels.length === 0}
                    isLoading={assignLoading}
                    size="sm"
                    onPress={handleAssignTunnel}
                  >
                    分配权限
                  </Button>
                </div>
              </section>

              <Divider className="opacity-60" />

              {/* 已有权限列表 */}
              <section className="space-y-3">
                <div className="flex items-center justify-between gap-2">
                  <h3 className="text-sm font-semibold text-foreground">
                    已有权限
                  </h3>
                  {tunnelListLoading && (
                    <div className="flex items-center gap-1.5 text-xs text-default-500">
                      <Spinner size="sm" />
                      加载中
                    </div>
                  )}
                </div>

                {tunnelListLoading && userTunnels.length === 0 ? (
                  <div className="flex items-center justify-center h-32">
                    <div className="flex items-center gap-2 text-sm text-default-500">
                      <Spinner size="sm" />
                      正在加载权限列表…
                    </div>
                  </div>
                ) : userTunnels.length === 0 ? (
                  <div className="text-center py-10 panel-card border border-divider bg-content1">
                    <div className="flex flex-col items-center gap-2">
                      <div className="empty-state-icon !w-12 !h-12">
                        <SettingsIcon className="w-5 h-5" />
                      </div>
                      <p className="text-sm font-medium text-foreground">
                        暂无隧道权限
                      </p>
                      <p className="text-xs text-default-500">
                        在上方表单选择隧道并分配即可
                      </p>
                    </div>
                  </div>
                ) : (
                  <div className="panel-table-wrap">
                    <table
                      aria-label="用户隧道权限列表"
                      className="w-full text-sm min-w-[820px]"
                    >
                      <thead>
                        <tr className="bg-default-50 text-default-600 font-medium text-xs">
                          <th className="px-3 py-2.5 text-left">隧道</th>
                          <th className="px-3 py-2.5 text-left min-w-[9rem]">
                            流量
                          </th>
                          <th className="px-3 py-2.5 text-left">转发</th>
                          <th className="px-3 py-2.5 text-left">状态</th>
                          <th className="px-3 py-2.5 text-left">限速</th>
                          <th className="px-3 py-2.5 text-left">重置</th>
                          <th className="px-3 py-2.5 text-left">有效期</th>
                          <th className="px-3 py-2.5 text-right">操作</th>
                        </tr>
                      </thead>
                      <tbody className="divide-y divide-divider">
                        {userTunnels.map((userTunnel) => {
                          const usedFlow = calculateTunnelUsedFlow(userTunnel);
                          const flowLimitBytes =
                            (userTunnel.flow || 0) * 1024 * 1024 * 1024;
                          const isUnlimited = userTunnel.flow === 99999;
                          const flowPercent =
                            !isUnlimited && flowLimitBytes > 0
                              ? Math.min((usedFlow / flowLimitBytes) * 100, 100)
                              : 0;
                          const expStatus = userTunnel.expTime
                            ? getExpireStatus(userTunnel.expTime)
                            : null;

                          return (
                            <tr
                              key={userTunnel.id}
                              className="bg-content1 hover:bg-default-50/70 transition-colors"
                            >
                              <td className={cellClass}>
                                <div className="min-w-0">
                                  <div className="font-medium text-foreground text-sm truncate">
                                    {userTunnel.tunnelName}
                                  </div>
                                  <div className="text-[11px] text-default-400 font-mono">
                                    #{userTunnel.tunnelId}
                                  </div>
                                </div>
                              </td>
                              <td className={cellClass}>
                                <div className="min-w-[8rem] max-w-[12rem]">
                                  <div className="flex justify-between text-xs mb-1 gap-2">
                                    <span className="text-default-500 truncate">
                                      {isUnlimited
                                        ? "无限制"
                                        : formatFlow(userTunnel.flow, "gb")}
                                    </span>
                                    <span className="font-mono text-danger whitespace-nowrap">
                                      {formatFlow(usedFlow)}
                                    </span>
                                  </div>
                                  <Progress
                                    aria-label={`${userTunnel.tunnelName} 流量 ${flowPercent.toFixed(1)}%`}
                                    color={
                                      flowPercent > 90
                                        ? "danger"
                                        : flowPercent > 70
                                          ? "warning"
                                          : "success"
                                    }
                                    size="sm"
                                    value={isUnlimited ? 0 : flowPercent}
                                  />
                                </div>
                              </td>
                              <td className={cellClass}>
                                <span className="text-xs font-medium">
                                  {userTunnel.num === 99999
                                    ? "无限制"
                                    : userTunnel.num}
                                </span>
                              </td>
                              <td className={cellClass}>
                                <Chip
                                  className="text-xs"
                                  color={
                                    userTunnel.status === 1
                                      ? "success"
                                      : "danger"
                                  }
                                  size="sm"
                                  startContent={
                                    <span
                                      className={`inline-block w-1.5 h-1.5 rounded-full ml-1 ${userTunnel.status === 1 ? "bg-success" : "bg-danger"}`}
                                    />
                                  }
                                  variant="flat"
                                >
                                  {userTunnel.status === 1 ? "正常" : "禁用"}
                                </Chip>
                              </td>
                              <td className={cellClass}>
                                <Chip
                                  className="text-xs"
                                  color={
                                    userTunnel.speedLimitName
                                      ? "warning"
                                      : "default"
                                  }
                                  size="sm"
                                  variant="flat"
                                >
                                  {userTunnel.speedLimitName || "不限速"}
                                </Chip>
                              </td>
                              <td className={cellClass}>
                                <span className="text-xs text-default-600 whitespace-nowrap">
                                  {userTunnel.flowResetTime === 0
                                    ? "不重置"
                                    : `每月${userTunnel.flowResetTime}号`}
                                </span>
                              </td>
                              <td className={cellClass}>
                                {userTunnel.expTime ? (
                                  <div className="space-y-1">
                                    <div className="text-xs text-default-600 whitespace-nowrap">
                                      {formatDate(userTunnel.expTime)}
                                    </div>
                                    {expStatus &&
                                      expStatus.color !== "success" && (
                                        <Chip
                                          className="text-xs"
                                          color={expStatus.color}
                                          size="sm"
                                          variant="flat"
                                        >
                                          {expStatus.text}
                                        </Chip>
                                      )}
                                  </div>
                                ) : (
                                  <span className="text-xs text-default-400">
                                    永久
                                  </span>
                                )}
                              </td>
                              <td className={cellClass}>
                                <div className="flex items-center gap-1 justify-end flex-wrap">
                                  <Button
                                    className="min-w-unit-12"
                                    color="primary"
                                    size="sm"
                                    startContent={
                                      <EditIcon className="w-3 h-3" />
                                    }
                                    variant="flat"
                                    onPress={() => handleEditTunnel(userTunnel)}
                                  >
                                    编辑
                                  </Button>
                                  <Button
                                    className="min-w-unit-12"
                                    color="warning"
                                    size="sm"
                                    variant="flat"
                                    onPress={() =>
                                      handleResetTunnelFlow(userTunnel)
                                    }
                                  >
                                    重置
                                  </Button>
                                  <Button
                                    className="min-w-unit-12"
                                    color="danger"
                                    size="sm"
                                    startContent={
                                      <DeleteIcon className="w-3 h-3" />
                                    }
                                    variant="flat"
                                    onPress={() =>
                                      handleRemoveTunnel(userTunnel)
                                    }
                                  >
                                    删除
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
              </section>
            </div>
          </ModalBody>
          <ModalFooter>
            <Button variant="light" onPress={onTunnelModalClose}>
              关闭
            </Button>
          </ModalFooter>
        </ModalContent>
      </Modal>

      {/* 编辑隧道权限模态框 */}
      <Modal
        backdrop="blur"
        isDismissable={false}
        isOpen={isEditTunnelModalOpen}
        placement="center"
        scrollBehavior="outside"
        size="2xl"
        onClose={onEditTunnelModalClose}
      >
        <ModalContent>
          <ModalHeader className="flex flex-col items-start gap-1 pb-2">
            <span className="text-lg font-semibold tracking-tight">
              编辑隧道权限
            </span>
            {editTunnelForm && (
              <p className="text-xs font-normal text-default-500">
                {editTunnelForm.tunnelName}
                <span className="text-default-400 font-mono ml-1">
                  #{editTunnelForm.tunnelId}
                </span>
              </p>
            )}
          </ModalHeader>
          <ModalBody>
            {editTunnelForm && (
              <div className="space-y-4">
                <div className="grid grid-cols-1 md:grid-cols-2 gap-3">
                  <Input
                    label="流量限制 (GB)"
                    max="99999"
                    min="1"
                    size="sm"
                    type="number"
                    value={editTunnelForm.flow.toString()}
                    onChange={(e) => {
                      const value = Math.min(
                        Math.max(Number(e.target.value) || 0, 1),
                        99999,
                      );

                      setEditTunnelForm((prev) =>
                        prev ? { ...prev, flow: value } : null,
                      );
                    }}
                  />

                  <Input
                    label="转发数量"
                    max="99999"
                    min="1"
                    size="sm"
                    type="number"
                    value={editTunnelForm.num.toString()}
                    onChange={(e) => {
                      const value = Math.min(
                        Math.max(Number(e.target.value) || 0, 1),
                        99999,
                      );

                      setEditTunnelForm((prev) =>
                        prev ? { ...prev, num: value } : null,
                      );
                    }}
                  />

                  <Select
                    label="限速规则"
                    selectedKeys={
                      editTunnelForm.speedId
                        ? [editTunnelForm.speedId.toString()]
                        : ["null"]
                    }
                    size="sm"
                    onSelectionChange={(keys) => {
                      const value = Array.from(keys)[0] as string;

                      setEditTunnelForm((prev) =>
                        prev
                          ? {
                              ...prev,
                              speedId: value === "null" ? null : Number(value),
                            }
                          : null,
                      );
                    }}
                  >
                    {[
                      <SelectItem key="null" textValue="不限速">
                        不限速
                      </SelectItem>,
                      ...editAvailableSpeedLimits.map((speedLimit) => (
                        <SelectItem
                          key={speedLimit.id.toString()}
                          textValue={speedLimit.name}
                        >
                          {speedLimit.name}
                        </SelectItem>
                      )),
                    ]}
                  </Select>

                  <Select
                    label="流量重置"
                    selectedKeys={[editTunnelForm.flowResetTime.toString()]}
                    size="sm"
                    onSelectionChange={(keys) => {
                      const value = Array.from(keys)[0] as string;

                      setEditTunnelForm((prev) =>
                        prev ? { ...prev, flowResetTime: Number(value) } : null,
                      );
                    }}
                  >
                    <>
                      <SelectItem key="0" textValue="不重置">
                        不重置
                      </SelectItem>
                      {Array.from({ length: 31 }, (_, i) => i + 1).map(
                        (day) => (
                          <SelectItem
                            key={day.toString()}
                            textValue={`每月${day}号（0点）`}
                          >
                            每月{day}号（0点）
                          </SelectItem>
                        ),
                      )}
                    </>
                  </Select>

                  <DatePicker
                    isRequired
                    showMonthAndYearPickers
                    className="cursor-pointer"
                    label="到期时间"
                    size="sm"
                    value={
                      editTunnelForm.expTime
                        ? (parseDate(
                            new Date(editTunnelForm.expTime)
                              .toISOString()
                              .split("T")[0],
                          ) as any)
                        : null
                    }
                    onChange={(date) => {
                      if (date) {
                        const jsDate = new Date(
                          date.year,
                          date.month - 1,
                          date.day,
                          23,
                          59,
                          59,
                        );

                        setEditTunnelForm((prev) =>
                          prev ? { ...prev, expTime: jsDate.getTime() } : null,
                        );
                      } else {
                        setEditTunnelForm((prev) =>
                          prev ? { ...prev, expTime: Date.now() } : null,
                        );
                      }
                    }}
                  />
                </div>

                <RadioGroup
                  label="状态"
                  orientation="horizontal"
                  size="sm"
                  value={editTunnelForm.status.toString()}
                  onValueChange={(value: string) =>
                    setEditTunnelForm((prev) =>
                      prev ? { ...prev, status: Number(value) } : null,
                    )
                  }
                >
                  <Radio value="1">正常</Radio>
                  <Radio value="0">禁用</Radio>
                </RadioGroup>
              </div>
            )}
          </ModalBody>
          <ModalFooter>
            <Button variant="light" onPress={onEditTunnelModalClose}>
              取消
            </Button>
            <Button
              color="primary"
              isLoading={editTunnelLoading}
              onPress={handleUpdateTunnel}
            >
              保存
            </Button>
          </ModalFooter>
        </ModalContent>
      </Modal>

      {/* 删除确认对话框 */}
      <Modal
        backdrop="blur"
        isOpen={isDeleteModalOpen}
        placement="center"
        scrollBehavior="outside"
        size="2xl"
        onClose={onDeleteModalClose}
      >
        <ModalContent>
          <ModalHeader className="flex flex-col gap-1">
            确认删除用户
          </ModalHeader>
          <ModalBody>
            <div className="flex items-center gap-4">
              <div className="w-12 h-12 bg-danger-100 rounded-full flex items-center justify-center">
                <DeleteIcon className="w-6 h-6 text-danger" />
              </div>
              <div className="flex-1">
                <p className="text-foreground">
                  确定要删除用户{" "}
                  <span className="font-semibold text-danger">
                    "{userToDelete?.user}"
                  </span>{" "}
                  吗？
                </p>
                <p className="text-small text-default-500 mt-1">
                  此操作不可撤销，用户的所有数据将被永久删除。
                </p>
              </div>
            </div>
          </ModalBody>
          <ModalFooter>
            <Button variant="light" onPress={onDeleteModalClose}>
              取消
            </Button>
            <Button color="danger" onPress={handleConfirmDelete}>
              确认删除
            </Button>
          </ModalFooter>
        </ModalContent>
      </Modal>

      {/* 删除隧道权限确认对话框 */}
      <Modal
        backdrop="blur"
        isOpen={isDeleteTunnelModalOpen}
        placement="center"
        scrollBehavior="outside"
        size="2xl"
        onClose={onDeleteTunnelModalClose}
      >
        <ModalContent>
          <ModalHeader className="flex flex-col gap-1">
            确认删除隧道权限
          </ModalHeader>
          <ModalBody>
            <div className="flex items-center gap-4">
              <div className="w-12 h-12 bg-danger-100 rounded-full flex items-center justify-center">
                <DeleteIcon className="w-6 h-6 text-danger" />
              </div>
              <div className="flex-1">
                <p className="text-foreground">
                  确定要删除用户{" "}
                  <span className="font-semibold">{currentUser?.user}</span>{" "}
                  对隧道{" "}
                  <span className="font-semibold text-danger">
                    "{tunnelToDelete?.tunnelName}"
                  </span>{" "}
                  的权限吗？
                </p>
                <p className="text-small text-default-500 mt-1">
                  删除后该用户将无法使用此隧道创建转发，此操作不可撤销。
                </p>
              </div>
            </div>
          </ModalBody>
          <ModalFooter>
            <Button variant="light" onPress={onDeleteTunnelModalClose}>
              取消
            </Button>
            <Button color="danger" onPress={handleConfirmRemoveTunnel}>
              确认删除
            </Button>
          </ModalFooter>
        </ModalContent>
      </Modal>

      {/* 重置流量确认对话框 */}
      <Modal
        backdrop="blur"
        isOpen={isResetFlowModalOpen}
        placement="center"
        scrollBehavior="outside"
        size="2xl"
        onClose={onResetFlowModalClose}
      >
        <ModalContent>
          <ModalHeader className="flex flex-col gap-1">
            确认重置流量
          </ModalHeader>
          <ModalBody>
            <div className="flex items-center gap-4">
              <div className="w-12 h-12 bg-warning-100 rounded-full flex items-center justify-center">
                <svg
                  className="w-6 h-6 text-warning"
                  fill="currentColor"
                  viewBox="0 0 20 20"
                >
                  <path
                    clipRule="evenodd"
                    d="M4 2a1 1 0 011 1v2.101a7.002 7.002 0 0111.601 2.566 1 1 0 11-1.885.666A5.002 5.002 0 005.999 7H9a1 1 0 010 2H4a1 1 0 01-1-1V3a1 1 0 011-1zm.008 9.057a1 1 0 011.276.61A5.002 5.002 0 0014.001 13H11a1 1 0 110-2h5a1 1 0 011 1v5a1 1 0 11-2 0v-2.101a7.002 7.002 0 01-11.601-2.566 1 1 0 01.61-1.276z"
                    fillRule="evenodd"
                  />
                </svg>
              </div>
              <div className="flex-1">
                <p className="text-foreground">
                  确定要重置用户{" "}
                  <span className="font-semibold text-warning">
                    "{userToReset?.user}"
                  </span>{" "}
                  的流量吗？
                </p>
                <p className="text-small text-default-500 mt-1">
                  该操作只会重置账号流量不会重置隧道权限流量，重置后该用户的上下行流量将归零，此操作不可撤销。
                </p>
                <div className="mt-2 p-2 bg-warning-50 dark:bg-warning-100/10 rounded text-xs">
                  <div className="text-warning-700 dark:text-warning-300">
                    当前流量使用情况：
                  </div>
                  <div className="mt-1 space-y-1">
                    <div className="flex justify-between">
                      <span>上行流量：</span>
                      <span className="font-mono">
                        {userToReset
                          ? formatFlow(userToReset.inFlow || 0)
                          : "-"}
                      </span>
                    </div>
                    <div className="flex justify-between">
                      <span>下行流量：</span>
                      <span className="font-mono">
                        {userToReset
                          ? formatFlow(userToReset.outFlow || 0)
                          : "-"}
                      </span>
                    </div>
                    <div className="flex justify-between font-medium">
                      <span>总计：</span>
                      <span className="font-mono text-warning-700 dark:text-warning-300">
                        {userToReset
                          ? formatFlow(calculateUserTotalUsedFlow(userToReset))
                          : "-"}
                      </span>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </ModalBody>
          <ModalFooter>
            <Button variant="light" onPress={onResetFlowModalClose}>
              取消
            </Button>
            <Button
              color="warning"
              isLoading={resetFlowLoading}
              onPress={handleConfirmResetFlow}
            >
              确认重置
            </Button>
          </ModalFooter>
        </ModalContent>
      </Modal>

      {/* 重置隧道流量确认对话框 */}
      <Modal
        backdrop="blur"
        isOpen={isResetTunnelFlowModalOpen}
        placement="center"
        scrollBehavior="outside"
        size="2xl"
        onClose={onResetTunnelFlowModalClose}
      >
        <ModalContent>
          <ModalHeader className="flex flex-col gap-1">
            确认重置隧道流量
          </ModalHeader>
          <ModalBody>
            <div className="flex items-center gap-4">
              <div className="w-12 h-12 bg-warning-100 rounded-full flex items-center justify-center">
                <svg
                  className="w-6 h-6 text-warning"
                  fill="currentColor"
                  viewBox="0 0 20 20"
                >
                  <path
                    clipRule="evenodd"
                    d="M4 2a1 1 0 011 1v2.101a7.002 7.002 0 0111.601 2.566 1 1 0 11-1.885.666A5.002 5.002 0 005.999 7H9a1 1 0 010 2H4a1 1 0 01-1-1V3a1 1 0 011-1zm.008 9.057a1 1 0 011.276.61A5.002 5.002 0 0014.001 13H11a1 1 0 110-2h5a1 1 0 011 1v5a1 1 0 11-2 0v-2.101a7.002 7.002 0 01-11.601-2.566 1 1 0 01.61-1.276z"
                    fillRule="evenodd"
                  />
                </svg>
              </div>
              <div className="flex-1">
                <p className="text-foreground">
                  确定要重置用户{" "}
                  <span className="font-semibold">{currentUser?.user}</span>{" "}
                  对隧道{" "}
                  <span className="font-semibold text-warning">
                    "{tunnelToReset?.tunnelName}"
                  </span>{" "}
                  的流量吗？
                </p>
                <p className="text-small text-default-500 mt-1">
                  该操作只会重置隧道权限流量不会重置账号流量，重置后该隧道权限的上下行流量将归零，此操作不可撤销。
                </p>
                <div className="mt-2 p-2 bg-warning-50 dark:bg-warning-100/10 rounded text-xs">
                  <div className="text-warning-700 dark:text-warning-300">
                    当前流量使用情况：
                  </div>
                  <div className="mt-1 space-y-1">
                    <div className="flex justify-between">
                      <span>上行流量：</span>
                      <span className="font-mono">
                        {tunnelToReset
                          ? formatFlow(tunnelToReset.inFlow || 0)
                          : "-"}
                      </span>
                    </div>
                    <div className="flex justify-between">
                      <span>下行流量：</span>
                      <span className="font-mono">
                        {tunnelToReset
                          ? formatFlow(tunnelToReset.outFlow || 0)
                          : "-"}
                      </span>
                    </div>
                    <div className="flex justify-between font-medium">
                      <span>总计：</span>
                      <span className="font-mono text-warning-700 dark:text-warning-300">
                        {tunnelToReset
                          ? formatFlow(calculateTunnelUsedFlow(tunnelToReset))
                          : "-"}
                      </span>
                    </div>
                  </div>
                </div>
              </div>
            </div>
          </ModalBody>
          <ModalFooter>
            <Button variant="light" onPress={onResetTunnelFlowModalClose}>
              取消
            </Button>
            <Button
              color="warning"
              isLoading={resetTunnelFlowLoading}
              onPress={handleConfirmResetTunnelFlow}
            >
              确认重置
            </Button>
          </ModalFooter>
        </ModalContent>
      </Modal>
    </div>
  );
}
