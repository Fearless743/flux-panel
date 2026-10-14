import { useState, useEffect } from "react";
import {
  Search,
  Pencil,
  Trash2,
  Plus,
  Settings2,
  RotateCcw,
  ChevronLeft,
  ChevronRight,
} from "lucide-react";
import toast from "react-hot-toast";

import {
  Button,
  Card,
  Field,
  Input,
  Select,
  Badge,
  Modal,
  PageHeader,
  Empty,
  Loading,
  Progress,
  Switch,
} from "@/components/ui";

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

const formatFlow = (value: number, unit: string = "bytes"): string => {
  if (unit === "gb") {
    return `${value} GB`;
  } else {
    if (!value || value <= 0) return "0 B";
    if (value < 1024) return `${value} B`;
    const k = 1024;
    const sizes = ["B", "KB", "MB", "GB", "TB", "PB", "EB"];
    const i = Math.min(Math.floor(Math.log(value) / Math.log(k)), sizes.length - 1);
    return `${(value / Math.pow(k, i)).toFixed(2)} ${sizes[i]}`;
  }
};

const formatDate = (timestamp: number): string => {
  return new Date(timestamp).toLocaleString();
};

const toDateInput = (d: Date | null): string => {
  if (!d) return "";
  const pad = (n: number) => String(n).padStart(2, "0");
  return `${d.getFullYear()}-${pad(d.getMonth() + 1)}-${pad(d.getDate())}`;
};

const fromDateInput = (v: string): Date | null => {
  if (!v) return null;
  const [y, m, d] = v.split("-").map(Number);
  if (!y || !m || !d) return null;
  return new Date(y, m - 1, d, 23, 59, 59);
};

const toDateInputTs = (ts: number | null): string => {
  if (!ts) return "";
  return toDateInput(new Date(ts));
};

const getExpireStatus = (expTime: number) => {
  const now = Date.now();
  if (expTime < now) {
    return { tone: "red" as const, text: "已过期" };
  }
  const diffDays = Math.ceil((expTime - now) / (1000 * 60 * 60 * 24));
  if (diffDays <= 7) {
    return { tone: "amber" as const, text: `${diffDays}天后过期` };
  }
  return { tone: "green" as const, text: "正常" };
};

const getUserStatus = (user: User) => {
  if (user.status === 1) {
    return { tone: "green" as const, text: "正常" };
  } else {
    return { tone: "red" as const, text: "禁用" };
  }
};

const calculateUserTotalUsedFlow = (user: User): number => {
  return (user.inFlow || 0) + (user.outFlow || 0);
};

const calculateTunnelUsedFlow = (tunnel: UserTunnel): number => {
  const inFlow = tunnel.inFlow || 0;
  const outFlow = tunnel.outFlow || 0;
  return inFlow + outFlow;
};

const UNLIMITED_FLOW = 99999;
const isUnlimitedFlow = (flow: number): boolean => flow >= UNLIMITED_FLOW;

const RESET_DAY_OPTIONS = ["0", ...Array.from({ length: 31 }, (_, i) => String(i + 1))];

function ResetDaySelect({
  value,
  onChange,
}: {
  value: number;
  onChange: (v: number) => void;
}) {
  return (
    <Select value={String(value)} onChange={(e) => onChange(Number(e.target.value))}>
      <option value="0">不重置</option>
      {Array.from({ length: 31 }, (_, i) => i + 1).map((day) => (
        <option key={day} value={String(day)}>
          每月{day}号（0点重置）
        </option>
      ))}
    </Select>
  );
}

export default function UserPage() {
  const [users, setUsers] = useState<User[]>([]);
  const [loading, setLoading] = useState(false);
  const [searchKeyword, setSearchKeyword] = useState("");
  const [pagination, setPagination] = useState<PaginationType>({
    current: 1,
    size: 10,
    total: 0,
  });

  const [isUserModalOpen, setUserModalOpen] = useState(false);
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

  const [isTunnelModalOpen, setTunnelModalOpen] = useState(false);
  const [currentUser, setCurrentUser] = useState<User | null>(null);
  const [userTunnels, setUserTunnels] = useState<UserTunnel[]>([]);
  const [tunnelListLoading, setTunnelListLoading] = useState(false);

  const [tunnelForm, setTunnelForm] = useState<UserTunnelForm>({
    tunnelId: null,
    flow: 100,
    num: 10,
    expTime: null,
    flowResetTime: 0,
    speedId: null,
  });
  const [assignLoading, setAssignLoading] = useState(false);

  const [isEditTunnelModalOpen, setEditTunnelModalOpen] = useState(false);
  const [editTunnelForm, setEditTunnelForm] = useState<UserTunnel | null>(null);
  const [editTunnelLoading, setEditTunnelLoading] = useState(false);

  const [isDeleteModalOpen, setDeleteModalOpen] = useState(false);
  const [userToDelete, setUserToDelete] = useState<User | null>(null);

  const [isDeleteTunnelModalOpen, setDeleteTunnelModalOpen] = useState(false);
  const [tunnelToDelete, setTunnelToDelete] = useState<UserTunnel | null>(null);

  const [isResetFlowModalOpen, setResetFlowModalOpen] = useState(false);
  const [userToReset, setUserToReset] = useState<User | null>(null);
  const [resetFlowLoading, setResetFlowLoading] = useState(false);

  const [isResetTunnelFlowModalOpen, setResetTunnelFlowModalOpen] = useState(false);
  const [tunnelToReset, setTunnelToReset] = useState<UserTunnel | null>(null);
  const [resetTunnelFlowLoading, setResetTunnelFlowLoading] = useState(false);

  const [tunnels, setTunnels] = useState<Tunnel[]>([]);
  const [speedLimits, setSpeedLimits] = useState<SpeedLimit[]>([]);

  useEffect(() => {
    loadTunnels();
    loadSpeedLimits();
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, []);

  useEffect(() => {
    const t = setTimeout(() => {
      loadUsers();
    }, 300);
    return () => clearTimeout(t);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [pagination.current, pagination.size, searchKeyword]);

  const loadUsers = async () => {
    setLoading(true);
    try {
      const response = await getAllUsers({
        current: pagination.current,
        size: pagination.size,
        keyword: searchKeyword,
      });

      if (response.code === 0) {
        const raw = response.data;
        if (Array.isArray(raw)) {
          setUsers(raw);
        } else if (raw && Array.isArray((raw as any).list)) {
          setUsers((raw as any).list);
          setPagination((prev) => ({ ...prev, total: (raw as any).total ?? prev.total }));
        } else if (raw && Array.isArray((raw as any).records)) {
          setUsers((raw as any).records);
          setPagination((prev) => ({ ...prev, total: (raw as any).total ?? prev.total }));
        } else {
          setUsers([]);
        }
      } else {
        toast.error(response.msg || "获取用户列表失败");
      }
    } catch {
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
    } catch {
      toast.error("获取隧道权限列表失败");
    } finally {
      setTunnelListLoading(false);
    }
  };

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
    setUserModalOpen(true);
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
    setUserModalOpen(true);
  };

  const handleDelete = (user: User) => {
    setUserToDelete(user);
    setDeleteModalOpen(true);
  };

  const handleConfirmDelete = async () => {
    if (!userToDelete) return;

    try {
      const response = await deleteUser(userToDelete.id);
      if (response.code === 0) {
        toast.success("删除成功");
        loadUsers();
        setDeleteModalOpen(false);
        setUserToDelete(null);
      } else {
        toast.error(response.msg || "删除失败");
      }
    } catch {
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
        expTime: (userForm.expTime as Date).getTime(),
      };

      if (isEdit && !submitData.pwd) {
        delete submitData.pwd;
      }

      const response = isEdit ? await updateUser(submitData) : await createUser(submitData);

      if (response.code === 0) {
        toast.success(isEdit ? "更新成功" : "创建成功");
        setUserModalOpen(false);
        loadUsers();
      } else {
        toast.error(response.msg || (isEdit ? "更新失败" : "创建失败"));
      }
    } catch {
      toast.error(isEdit ? "更新失败" : "创建失败");
    } finally {
      setUserFormLoading(false);
    }
  };

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
    setTunnelModalOpen(true);
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
        expTime: (tunnelForm.expTime as Date).getTime(),
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
    } catch {
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
    setEditTunnelModalOpen(true);
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
        setEditTunnelModalOpen(false);
        if (currentUser) {
          loadUserTunnels(currentUser.id);
        }
      } else {
        toast.error(response.msg || "更新失败");
      }
    } catch {
      toast.error("更新失败");
    } finally {
      setEditTunnelLoading(false);
    }
  };

  const handleRemoveTunnel = (userTunnel: UserTunnel) => {
    setTunnelToDelete(userTunnel);
    setDeleteTunnelModalOpen(true);
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
        setDeleteTunnelModalOpen(false);
        setTunnelToDelete(null);
      } else {
        toast.error(response.msg || "删除失败");
      }
    } catch {
      toast.error("删除失败");
    }
  };

  const handleResetFlow = (user: User) => {
    setUserToReset(user);
    setResetFlowModalOpen(true);
  };

  const handleConfirmResetFlow = async () => {
    if (!userToReset) return;

    setResetFlowLoading(true);
    try {
      const response = await resetUserFlow({
        id: userToReset.id,
        type: 1,
      });

      if (response.code === 0) {
        toast.success("流量重置成功");
        setResetFlowModalOpen(false);
        setUserToReset(null);
        loadUsers();
      } else {
        toast.error(response.msg || "重置失败");
      }
    } catch {
      toast.error("重置失败");
    } finally {
      setResetFlowLoading(false);
    }
  };

  const handleResetTunnelFlow = (userTunnel: UserTunnel) => {
    setTunnelToReset(userTunnel);
    setResetTunnelFlowModalOpen(true);
  };

  const handleConfirmResetTunnelFlow = async () => {
    if (!tunnelToReset) return;

    setResetTunnelFlowLoading(true);
    try {
      const response = await resetUserFlow({
        id: tunnelToReset.id,
        type: 2,
      });

      if (response.code === 0) {
        toast.success("隧道流量重置成功");
        setResetTunnelFlowModalOpen(false);
        setTunnelToReset(null);
        if (currentUser) {
          loadUserTunnels(currentUser.id);
        }
      } else {
        toast.error(response.msg || "重置失败");
      }
    } catch {
      toast.error("重置失败");
    } finally {
      setResetTunnelFlowLoading(false);
    }
  };

  const availableTunnels = tunnels.filter(
    (tunnel) => !userTunnels.some((ut) => ut.tunnelId === tunnel.id)
  );

  const availableSpeedLimits = speedLimits.filter(
    (speedLimit) => speedLimit.tunnelId === tunnelForm.tunnelId
  );

  const editAvailableSpeedLimits = speedLimits.filter(
    (speedLimit) => speedLimit.tunnelId === editTunnelForm?.tunnelId
  );

  const activeCount = users.filter((u) => u.status === 1).length;
  const totalPages = Math.max(1, Math.ceil((pagination.total || users.length || 1) / pagination.size));

  return (
    <div>
      <PageHeader
        title="用户管理"
        desc={`共 ${pagination.total || users.length} 个用户${
          users.length > 0 ? ` · 正常 ${activeCount} · 禁用 ${users.length - activeCount}` : ""
        }`}
        actions={
          <Button variant="primary" size="sm" onClick={handleAdd}>
            <Plus size={15} /> 新增
          </Button>
        }
      />

      <div className="mb-4 flex flex-col gap-3 sm:flex-row sm:items-center">
        <div className="flex max-w-md flex-1 items-center gap-2">
          <div className="relative flex-1">
            <Search
              size={15}
              className="pointer-events-none absolute left-3 top-1/2 -translate-y-1/2 text-slate-500"
            />
            <Input
              value={searchKeyword}
              onChange={(e) => setSearchKeyword(e.target.value)}
              placeholder="搜索用户名"
              onKeyDown={(e) => e.key === "Enter" && handleSearch()}
              className="pl-9"
            />
          </div>
          <Button variant="soft" size="sm" onClick={handleSearch}>
            搜索
          </Button>
        </div>
      </div>

      <Card>
        {loading ? (
          <Loading text="正在加载..." />
        ) : users.length === 0 ? (
          <Empty
            text="暂无用户数据，点击右上按钮开始创建"
            action={
              <Button variant="primary" size="sm" onClick={handleAdd}>
                <Plus size={15} /> 新增用户
              </Button>
            }
          />
        ) : (
          <>
            <div className="overflow-x-auto">
              <table className="flux-table min-w-[960px]" aria-label="用户列表">
                <thead>
                  <tr>
                    <th className="text-left">用户</th>
                    <th className="text-left">状态</th>
                    <th className="text-left">流量</th>
                    <th className="text-left">转发配额</th>
                    <th className="text-left">重置</th>
                    <th className="text-left">有效期</th>
                    <th className="text-right">操作</th>
                  </tr>
                </thead>
                <tbody>
                  {users.map((user) => {
                    const userStatus = getUserStatus(user);
                    const expStatus = user.expTime ? getExpireStatus(user.expTime) : null;
                    const usedFlow = calculateUserTotalUsedFlow(user);
                    const flowLimitBytes = (user.flow || 0) * 1024 * 1024 * 1024;
                    const isUnlimited = isUnlimitedFlow(user.flow);
                    const flowPercent =
                      !isUnlimited && flowLimitBytes > 0
                        ? Math.min((usedFlow / flowLimitBytes) * 100, 100)
                        : 0;

                    return (
                      <tr key={user.id}>
                        <td>
                          <div className="min-w-0">
                            <div className="truncate text-sm font-medium text-white">
                              {user.name || user.user}
                            </div>
                            <div className="truncate text-xs text-slate-500">@{user.user}</div>
                          </div>
                        </td>
                        <td>
                          <Badge tone={userStatus.tone} dot>
                            {userStatus.text}
                          </Badge>
                        </td>
                        <td>
                          <div className="min-w-[9rem] max-w-[14rem]">
                            <div className="mb-1 flex justify-between gap-2 text-xs">
                              <span className="truncate text-slate-400">
                                {isUnlimited ? "无限制" : formatFlow(user.flow, "gb")}
                              </span>
                              <span className="whitespace-nowrap font-mono text-red-300">
                                {formatFlow(usedFlow)}
                              </span>
                            </div>
                            <Progress value={isUnlimited ? 0 : flowPercent} />
                            <div className="mt-0.5 text-[11px] text-slate-500">
                              {isUnlimited ? "已用流量" : `已用 ${flowPercent.toFixed(1)}%`}
                            </div>
                          </div>
                        </td>
                        <td>
                          <span className="text-xs font-medium text-slate-200">
                            {user.num === 99999 ? "无限制" : user.num}
                          </span>
                        </td>
                        <td>
                          <span className="whitespace-nowrap text-xs text-slate-400">
                            {user.flowResetTime === 0 ? "不重置" : `每月${user.flowResetTime}号`}
                          </span>
                        </td>
                        <td>
                          {user.expTime ? (
                            <div className="space-y-1">
                              <div className="whitespace-nowrap text-xs text-slate-400">
                                {formatDate(user.expTime)}
                              </div>
                              {expStatus && expStatus.tone !== "green" && (
                                <Badge tone={expStatus.tone}>{expStatus.text}</Badge>
                              )}
                            </div>
                          ) : (
                            <span className="text-xs text-slate-500">永久</span>
                          )}
                        </td>
                        <td>
                          <div className="flex items-center justify-end gap-1.5">
                            <Button size="sm" variant="soft" onClick={() => handleEdit(user)} title="编辑">
                              <Pencil size={13} /> 编辑
                            </Button>
                            <Button
                              size="sm"
                              variant="outline"
                              onClick={() => handleResetFlow(user)}
                              title="重置账号流量"
                            >
                              <RotateCcw size={13} /> 重置
                            </Button>
                            <Button
                              size="sm"
                              variant="ghost"
                              onClick={() => handleManageTunnels(user)}
                              title="隧道权限"
                            >
                              <Settings2 size={13} /> 权限
                            </Button>
                            <Button size="sm" variant="danger" onClick={() => handleDelete(user)} title="删除">
                              <Trash2 size={13} />
                            </Button>
                          </div>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
            <div className="flex flex-wrap items-center justify-between gap-3 border-t border-white/[0.06] px-4 py-3">
              <div className="flex items-center gap-2 text-xs text-slate-400">
                <span>每页</span>
                <Select
                  className="w-20"
                  value={String(pagination.size)}
                  onChange={(e) =>
                    setPagination((prev) => ({
                      ...prev,
                      size: Number(e.target.value),
                      current: 1,
                    }))
                  }
                >
                  {[10, 20, 50, 100].map((s) => (
                    <option key={s} value={String(s)}>
                      {s}
                    </option>
                  ))}
                </Select>
                <span>条</span>
              </div>
              <div className="flex items-center gap-2 text-xs text-slate-400">
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={pagination.current <= 1}
                  onClick={() =>
                    setPagination((prev) => ({ ...prev, current: Math.max(1, prev.current - 1) }))
                  }
                >
                  <ChevronLeft size={14} />
                </Button>
                <span>
                  第 {pagination.current} / {totalPages} 页
                </span>
                <Button
                  size="sm"
                  variant="ghost"
                  disabled={pagination.current >= totalPages}
                  onClick={() =>
                    setPagination((prev) => ({ ...prev, current: prev.current + 1 }))
                  }
                >
                  <ChevronRight size={14} />
                </Button>
              </div>
            </div>
          </>
        )}
      </Card>

      <Modal
        open={isUserModalOpen}
        onClose={() => setUserModalOpen(false)}
        title={isEdit ? "编辑用户" : "新增用户"}
        width="max-w-2xl"
        footer={
          <>
            <Button variant="ghost" onClick={() => setUserModalOpen(false)}>
              取消
            </Button>
            <Button variant="primary" onClick={handleSubmitUser} loading={userFormLoading}>
              确定
            </Button>
          </>
        }
      >
        <div className="grid grid-cols-1 gap-4 md:grid-cols-2">
          <Field label="用户名 *">
            <Input
              value={userForm.user}
              onChange={(e) => setUserForm((prev) => ({ ...prev, user: e.target.value }))}
              placeholder="请输入用户名"
            />
          </Field>
          <Field label={isEdit ? "密码（留空不修改）" : "密码 *"}>
            <Input
              type="password"
              value={userForm.pwd || ""}
              onChange={(e) => setUserForm((prev) => ({ ...prev, pwd: e.target.value }))}
              placeholder={isEdit ? "留空则不修改密码" : "请输入密码"}
            />
          </Field>
          <Field label="流量限制 (GB) *">
            <Input
              type="number"
              min={1}
              max={99999}
              disabled={isUnlimitedFlow(userForm.flow)}
              placeholder={isUnlimitedFlow(userForm.flow) ? "无限制" : "请输入流量"}
              value={isUnlimitedFlow(userForm.flow) ? "" : String(userForm.flow)}
              onChange={(e) => {
                const value = Math.min(Math.max(Number(e.target.value) || 0, 1), 99999);
                setUserForm((prev) => ({ ...prev, flow: value }));
              }}
            />
          </Field>
          <Field label="转发数量 *">
            <Input
              type="number"
              min={1}
              max={99999}
              value={String(userForm.num)}
              onChange={(e) => {
                const value = Math.min(Math.max(Number(e.target.value) || 0, 1), 99999);
                setUserForm((prev) => ({ ...prev, num: value }));
              }}
            />
          </Field>
          <Field label="流量重置日期">
            <ResetDaySelect
              value={userForm.flowResetTime ?? 0}
              onChange={(v) => setUserForm((prev) => ({ ...prev, flowResetTime: v }))}
            />
          </Field>
          <Field label="过期时间 *">
            <Input
              type="date"
              value={userForm.expTime ? toDateInput(userForm.expTime as Date) : ""}
              onChange={(e) =>
                setUserForm((prev) => ({ ...prev, expTime: fromDateInput(e.target.value) }))
              }
            />
          </Field>
        </div>

        <div className="mt-4 flex items-center justify-between gap-3 rounded-xl border border-white/[0.07] bg-white/[0.03] px-4 py-3">
          <div>
            <p className="text-sm font-medium text-white">无限流量</p>
            <p className="mt-0.5 text-xs text-slate-500">开启后该用户流量不受限制</p>
          </div>
          <Switch
            checked={isUnlimitedFlow(userForm.flow)}
            onChange={(selected) =>
              setUserForm((prev) => ({ ...prev, flow: selected ? UNLIMITED_FLOW : 100 }))
            }
          />
        </div>

        <div className="mt-4 flex items-center gap-4">
          <span className="text-[0.78rem] font-medium text-slate-300">状态</span>
          {(["1", "0"] as const).map((v) => (
            <label key={v} className="flex cursor-pointer items-center gap-1.5 text-sm text-slate-300">
              <input
                type="radio"
                name="user-status"
                value={v}
                checked={String(userForm.status) === v}
                onChange={() => setUserForm((prev) => ({ ...prev, status: Number(v) }))}
                className="h-4 w-4 accent-blue-400"
              />
              {v === "1" ? "正常" : "禁用"}
            </label>
          ))}
        </div>
      </Modal>

      <Modal
        open={isTunnelModalOpen}
        onClose={() => setTunnelModalOpen(false)}
        title={`隧道权限${currentUser ? ` · @${currentUser.user}` : ""}`}
        desc={`已分配 ${userTunnels.length} 条${
          availableTunnels.length > 0
            ? ` · 还可分配 ${availableTunnels.length} 条`
            : tunnels.length > 0
              ? " · 全部隧道已分配"
              : " · 暂无可用隧道"
        }`}
        width="max-w-5xl"
        footer={
          <Button variant="ghost" onClick={() => setTunnelModalOpen(false)}>
            关闭
          </Button>
        }
      >
        <div className="space-y-5">
          <section className="space-y-3 rounded-2xl border border-white/[0.07] bg-white/[0.02] p-4">
            <div className="flex items-center justify-between gap-2">
              <div>
                <h3 className="text-sm font-semibold text-white">分配新权限</h3>
                <p className="mt-0.5 text-xs text-slate-500">选择未分配隧道并设置配额、限速与有效期</p>
              </div>
              {availableTunnels.length === 0 && <Badge tone="slate">无可分配</Badge>}
            </div>

            <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3">
              <Field label="隧道">
                <Select
                  value={tunnelForm.tunnelId ? String(tunnelForm.tunnelId) : ""}
                  disabled={availableTunnels.length === 0}
                  onChange={(e) => {
                    const v = e.target.value;
                    setTunnelForm((prev) => ({
                      ...prev,
                      tunnelId: v ? Number(v) : null,
                      speedId: null,
                    }));
                  }}
                >
                  <option value="">选择隧道</option>
                  {availableTunnels.map((tunnel) => (
                    <option key={tunnel.id} value={String(tunnel.id)}>
                      {tunnel.name}
                    </option>
                  ))}
                </Select>
              </Field>

              <Field label="限速规则">
                <Select
                  value={tunnelForm.speedId ? String(tunnelForm.speedId) : "null"}
                  disabled={!tunnelForm.tunnelId}
                  onChange={(e) => {
                    const v = e.target.value;
                    setTunnelForm((prev) => ({
                      ...prev,
                      speedId: v === "null" ? null : Number(v),
                    }));
                  }}
                >
                  <option value="null">不限速</option>
                  {availableSpeedLimits.map((sl) => (
                    <option key={sl.id} value={String(sl.id)}>
                      {sl.name}
                    </option>
                  ))}
                </Select>
              </Field>

              <Field label="流量限制 (GB)">
                <Input
                  type="number"
                  min={1}
                  max={99999}
                  disabled={isUnlimitedFlow(tunnelForm.flow)}
                  placeholder={isUnlimitedFlow(tunnelForm.flow) ? "无限制" : "请输入流量"}
                  value={isUnlimitedFlow(tunnelForm.flow) ? "" : String(tunnelForm.flow)}
                  onChange={(e) => {
                    const value = Math.min(Math.max(Number(e.target.value) || 0, 1), 99999);
                    setTunnelForm((prev) => ({ ...prev, flow: value }));
                  }}
                />
              </Field>

              <Field label="转发数量">
                <Input
                  type="number"
                  min={1}
                  max={99999}
                  value={String(tunnelForm.num)}
                  onChange={(e) => {
                    const value = Math.min(Math.max(Number(e.target.value) || 0, 1), 99999);
                    setTunnelForm((prev) => ({ ...prev, num: value }));
                  }}
                />
              </Field>

              <Field label="流量重置">
                <ResetDaySelect
                  value={tunnelForm.flowResetTime ?? 0}
                  onChange={(v) => setTunnelForm((prev) => ({ ...prev, flowResetTime: v }))}
                />
              </Field>

              <Field label="到期时间">
                <Input
                  type="date"
                  value={tunnelForm.expTime ? toDateInput(tunnelForm.expTime as Date) : ""}
                  onChange={(e) =>
                    setTunnelForm((prev) => ({ ...prev, expTime: fromDateInput(e.target.value) }))
                  }
                />
              </Field>

              <div className="col-span-1 flex items-center justify-between gap-3 rounded-xl border border-white/[0.07] bg-white/[0.03] px-4 py-2.5 sm:col-span-2 lg:col-span-3">
                <div>
                  <p className="text-sm font-medium text-white">无限流量</p>
                  <p className="mt-0.5 text-xs text-slate-500">开启后该隧道权限流量不受限制</p>
                </div>
                <Switch
                  checked={isUnlimitedFlow(tunnelForm.flow)}
                  onChange={(selected) =>
                    setTunnelForm((prev) => ({
                      ...prev,
                      flow: selected ? UNLIMITED_FLOW : 100,
                    }))
                  }
                />
              </div>
            </div>

            <div className="flex justify-end">
              <Button
                variant="primary"
                size="sm"
                onClick={handleAssignTunnel}
                loading={assignLoading}
                disabled={availableTunnels.length === 0}
              >
                分配权限
              </Button>
            </div>
          </section>

          <section className="space-y-3">
            <div className="flex items-center justify-between gap-2">
              <h3 className="text-sm font-semibold text-white">已有权限</h3>
              {tunnelListLoading && <span className="text-xs text-slate-500">加载中…</span>}
            </div>

            {tunnelListLoading && userTunnels.length === 0 ? (
              <Loading text="正在加载权限列表…" />
            ) : userTunnels.length === 0 ? (
              <Card>
                <Empty text="暂无隧道权限，在上方表单分配即可" />
              </Card>
            ) : (
              <div className="overflow-x-auto rounded-2xl border border-white/[0.07]">
                <table className="flux-table min-w-[820px]" aria-label="用户隧道权限列表">
                  <thead>
                    <tr>
                      <th className="text-left">隧道</th>
                      <th className="text-left">流量</th>
                      <th className="text-left">转发</th>
                      <th className="text-left">状态</th>
                      <th className="text-left">限速</th>
                      <th className="text-left">重置</th>
                      <th className="text-left">有效期</th>
                      <th className="text-right">操作</th>
                    </tr>
                  </thead>
                  <tbody>
                    {userTunnels.map((userTunnel) => {
                      const usedFlow = calculateTunnelUsedFlow(userTunnel);
                      const flowLimitBytes = (userTunnel.flow || 0) * 1024 * 1024 * 1024;
                      const isUnlimited = isUnlimitedFlow(userTunnel.flow);
                      const flowPercent =
                        !isUnlimited && flowLimitBytes > 0
                          ? Math.min((usedFlow / flowLimitBytes) * 100, 100)
                          : 0;
                      const expStatus = userTunnel.expTime
                        ? getExpireStatus(userTunnel.expTime)
                        : null;

                      return (
                        <tr key={userTunnel.id}>
                          <td>
                            <div className="min-w-0">
                              <div className="truncate text-sm font-medium text-white">
                                {userTunnel.tunnelName}
                              </div>
                              <div className="font-mono text-[11px] text-slate-500">
                                #{userTunnel.tunnelId}
                              </div>
                            </div>
                          </td>
                          <td>
                            <div className="min-w-[8rem] max-w-[12rem]">
                              <div className="mb-1 flex justify-between gap-2 text-xs">
                                <span className="truncate text-slate-400">
                                  {isUnlimited ? "无限制" : formatFlow(userTunnel.flow, "gb")}
                                </span>
                                <span className="whitespace-nowrap font-mono text-red-300">
                                  {formatFlow(usedFlow)}
                                </span>
                              </div>
                              <Progress value={isUnlimited ? 0 : flowPercent} />
                            </div>
                          </td>
                          <td>
                            <span className="text-xs font-medium text-slate-200">
                              {userTunnel.num === 99999 ? "无限制" : userTunnel.num}
                            </span>
                          </td>
                          <td>
                            <Badge tone={userTunnel.status === 1 ? "green" : "red"} dot>
                              {userTunnel.status === 1 ? "正常" : "禁用"}
                            </Badge>
                          </td>
                          <td>
                            <Badge tone={userTunnel.speedLimitName ? "amber" : "slate"}>
                              {userTunnel.speedLimitName || "不限速"}
                            </Badge>
                          </td>
                          <td>
                            <span className="whitespace-nowrap text-xs text-slate-400">
                              {userTunnel.flowResetTime === 0
                                ? "不重置"
                                : `每月${userTunnel.flowResetTime}号`}
                            </span>
                          </td>
                          <td>
                            {userTunnel.expTime ? (
                              <div className="space-y-1">
                                <div className="whitespace-nowrap text-xs text-slate-400">
                                  {formatDate(userTunnel.expTime)}
                                </div>
                                {expStatus && expStatus.tone !== "green" && (
                                  <Badge tone={expStatus.tone}>{expStatus.text}</Badge>
                                )}
                              </div>
                            ) : (
                              <span className="text-xs text-slate-500">永久</span>
                            )}
                          </td>
                          <td>
                            <div className="flex items-center justify-end gap-1.5">
                              <Button
                                size="sm"
                                variant="soft"
                                onClick={() => handleEditTunnel(userTunnel)}
                              >
                                <Pencil size={12} /> 编辑
                              </Button>
                              <Button
                                size="sm"
                                variant="outline"
                                onClick={() => handleResetTunnelFlow(userTunnel)}
                              >
                                <RotateCcw size={12} /> 重置
                              </Button>
                              <Button
                                size="sm"
                                variant="danger"
                                onClick={() => handleRemoveTunnel(userTunnel)}
                              >
                                <Trash2 size={12} />
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
      </Modal>

      <Modal
        open={isEditTunnelModalOpen}
        onClose={() => setEditTunnelModalOpen(false)}
        title="编辑隧道权限"
        desc={
          editTunnelForm
            ? `${editTunnelForm.tunnelName} · #${editTunnelForm.tunnelId}`
            : undefined
        }
        width="max-w-2xl"
        footer={
          <>
            <Button variant="ghost" onClick={() => setEditTunnelModalOpen(false)}>
              取消
            </Button>
            <Button variant="primary" onClick={handleUpdateTunnel} loading={editTunnelLoading}>
              保存
            </Button>
          </>
        }
      >
        {editTunnelForm && (
          <div className="space-y-4">
            <div className="grid grid-cols-1 gap-3 md:grid-cols-2">
              <Field label="流量限制 (GB)">
                <Input
                  type="number"
                  min={1}
                  max={99999}
                  disabled={isUnlimitedFlow(editTunnelForm.flow)}
                  placeholder={isUnlimitedFlow(editTunnelForm.flow) ? "无限制" : "请输入流量"}
                  value={
                    isUnlimitedFlow(editTunnelForm.flow) ? "" : String(editTunnelForm.flow)
                  }
                  onChange={(e) => {
                    const value = Math.min(Math.max(Number(e.target.value) || 0, 1), 99999);
                    setEditTunnelForm((prev) => (prev ? { ...prev, flow: value } : null));
                  }}
                />
              </Field>

              <Field label="转发数量">
                <Input
                  type="number"
                  min={1}
                  max={99999}
                  value={String(editTunnelForm.num)}
                  onChange={(e) => {
                    const value = Math.min(Math.max(Number(e.target.value) || 0, 1), 99999);
                    setEditTunnelForm((prev) => (prev ? { ...prev, num: value } : null));
                  }}
                />
              </Field>

              <Field label="限速规则">
                <Select
                  value={editTunnelForm.speedId ? String(editTunnelForm.speedId) : "null"}
                  onChange={(e) => {
                    const v = e.target.value;
                    setEditTunnelForm((prev) =>
                      prev ? { ...prev, speedId: v === "null" ? null : Number(v) } : null
                    );
                  }}
                >
                  <option value="null">不限速</option>
                  {editAvailableSpeedLimits.map((sl) => (
                    <option key={sl.id} value={String(sl.id)}>
                      {sl.name}
                    </option>
                  ))}
                </Select>
              </Field>

              <Field label="流量重置">
                <ResetDaySelect
                  value={editTunnelForm.flowResetTime ?? 0}
                  onChange={(v) =>
                    setEditTunnelForm((prev) => (prev ? { ...prev, flowResetTime: v } : null))
                  }
                />
              </Field>

              <Field label="到期时间 *">
                <Input
                  type="date"
                  value={toDateInputTs(editTunnelForm.expTime)}
                  onChange={(e) => {
                    const d = fromDateInput(e.target.value);
                    setEditTunnelForm((prev) =>
                      prev ? { ...prev, expTime: d ? d.getTime() : Date.now() } : null
                    );
                  }}
                />
              </Field>

              <div className="col-span-1 flex items-center justify-between gap-3 rounded-xl border border-white/[0.07] bg-white/[0.03] px-4 py-2.5 md:col-span-2">
                <div>
                  <p className="text-sm font-medium text-white">无限流量</p>
                  <p className="mt-0.5 text-xs text-slate-500">开启后该隧道权限流量不受限制</p>
                </div>
                <Switch
                  checked={isUnlimitedFlow(editTunnelForm.flow)}
                  onChange={(selected) =>
                    setEditTunnelForm((prev) =>
                      prev ? { ...prev, flow: selected ? UNLIMITED_FLOW : 100 } : null
                    )
                  }
                />
              </div>
            </div>

            <div className="flex items-center gap-4">
              <span className="text-[0.78rem] font-medium text-slate-300">状态</span>
              {(["1", "0"] as const).map((v) => (
                <label
                  key={v}
                  className="flex cursor-pointer items-center gap-1.5 text-sm text-slate-300"
                >
                  <input
                    type="radio"
                    name="tunnel-status"
                    value={v}
                    checked={String(editTunnelForm.status) === v}
                    onChange={() =>
                      setEditTunnelForm((prev) => (prev ? { ...prev, status: Number(v) } : null))
                    }
                    className="h-4 w-4 accent-blue-400"
                  />
                  {v === "1" ? "正常" : "禁用"}
                </label>
              ))}
            </div>
          </div>
        )}
      </Modal>

      <Modal
        open={isDeleteModalOpen}
        onClose={() => setDeleteModalOpen(false)}
        title="确认删除用户"
        desc="此操作不可撤销，用户的所有数据将被永久删除"
        footer={
          <>
            <Button variant="ghost" onClick={() => setDeleteModalOpen(false)}>
              取消
            </Button>
            <Button variant="danger" onClick={handleConfirmDelete}>
              确认删除
            </Button>
          </>
        }
      >
        <p className="text-sm text-slate-300">
          确定要删除用户 <span className="font-semibold text-red-300">"{userToDelete?.user}"</span>{" "}
          吗？
        </p>
      </Modal>

      <Modal
        open={isDeleteTunnelModalOpen}
        onClose={() => setDeleteTunnelModalOpen(false)}
        title="确认删除隧道权限"
        desc="删除后该用户将无法使用此隧道创建转发，此操作不可撤销"
        footer={
          <>
            <Button variant="ghost" onClick={() => setDeleteTunnelModalOpen(false)}>
              取消
            </Button>
            <Button variant="danger" onClick={handleConfirmRemoveTunnel}>
              确认删除
            </Button>
          </>
        }
      >
        <p className="text-sm text-slate-300">
          确定要删除用户 <span className="font-semibold text-white">{currentUser?.user}</span> 对隧道{" "}
          <span className="font-semibold text-red-300">"{tunnelToDelete?.tunnelName}"</span>{" "}
          的权限吗？
        </p>
      </Modal>

      <Modal
        open={isResetFlowModalOpen}
        onClose={() => setResetFlowModalOpen(false)}
        title="确认重置流量"
        desc="只重置账号流量，不重置隧道权限流量；上下行流量将归零，不可撤销"
        footer={
          <>
            <Button variant="ghost" onClick={() => setResetFlowModalOpen(false)}>
              取消
            </Button>
            <Button variant="primary" onClick={handleConfirmResetFlow} loading={resetFlowLoading}>
              确认重置
            </Button>
          </>
        }
      >
        <p className="text-sm text-slate-300">
          确定要重置用户 <span className="font-semibold text-amber-300">"{userToReset?.user}"</span>{" "}
          的流量吗？
        </p>
        <div className="mt-2 space-y-1 rounded-xl border border-white/[0.07] bg-white/[0.03] p-3 text-xs text-slate-300">
          <div className="flex justify-between">
            <span>上行流量：</span>
            <span className="font-mono">{userToReset ? formatFlow(userToReset.inFlow || 0) : "-"}</span>
          </div>
          <div className="flex justify-between">
            <span>下行流量：</span>
            <span className="font-mono">{userToReset ? formatFlow(userToReset.outFlow || 0) : "-"}</span>
          </div>
          <div className="flex justify-between font-medium">
            <span>总计：</span>
            <span className="font-mono text-amber-300">
              {userToReset ? formatFlow(calculateUserTotalUsedFlow(userToReset)) : "-"}
            </span>
          </div>
        </div>
      </Modal>

      <Modal
        open={isResetTunnelFlowModalOpen}
        onClose={() => setResetTunnelFlowModalOpen(false)}
        title="确认重置隧道流量"
        desc="只重置隧道权限流量，不重置账号流量；不可撤销"
        footer={
          <>
            <Button variant="ghost" onClick={() => setResetTunnelFlowModalOpen(false)}>
              取消
            </Button>
            <Button
              variant="primary"
              onClick={handleConfirmResetTunnelFlow}
              loading={resetTunnelFlowLoading}
            >
              确认重置
            </Button>
          </>
        }
      >
        <p className="text-sm text-slate-300">
          确定要重置用户 <span className="font-semibold text-white">{currentUser?.user}</span> 对隧道{" "}
          <span className="font-semibold text-amber-300">"{tunnelToReset?.tunnelName}"</span>{" "}
          的流量吗？
        </p>
        <div className="mt-2 space-y-1 rounded-xl border border-white/[0.07] bg-white/[0.03] p-3 text-xs text-slate-300">
          <div className="flex justify-between">
            <span>上行流量：</span>
            <span className="font-mono">
              {tunnelToReset ? formatFlow(tunnelToReset.inFlow || 0) : "-"}
            </span>
          </div>
          <div className="flex justify-between">
            <span>下行流量：</span>
            <span className="font-mono">
              {tunnelToReset ? formatFlow(tunnelToReset.outFlow || 0) : "-"}
            </span>
          </div>
          <div className="flex justify-between font-medium">
            <span>总计：</span>
            <span className="font-mono text-amber-300">
              {tunnelToReset ? formatFlow(calculateTunnelUsedFlow(tunnelToReset)) : "-"}
            </span>
          </div>
        </div>
      </Modal>

      {/* keep RESET_DAY_OPTIONS referenced for parity */}
      <span className="hidden">{RESET_DAY_OPTIONS.length}</span>
    </div>
  );
}
