import { useState, useEffect } from "react";
import { Pencil, Trash2, Plus, Gauge } from "lucide-react";
import toast from "react-hot-toast";

import {
  Button,
  Card,
  CardHead,
  Field,
  Input,
  Select,
  Badge,
  Modal,
  PageHeader,
  Empty,
  Loading,
} from "@/components/ui";

import {
  createSpeedLimit,
  getSpeedLimitList,
  updateSpeedLimit,
  deleteSpeedLimit,
  getTunnelList,
} from "@/api";

interface SpeedLimitRule {
  id: number;
  name: string;
  speed: number;
  status: number;
  tunnelId: number;
  tunnelName: string;
  createdTime: string;
  updatedTime: string;
}

interface Tunnel {
  id: number;
  name: string;
}

interface SpeedLimitForm {
  id?: number;
  name: string;
  speed: number;
  tunnelId: number | null;
  tunnelName: string;
  status: number;
}

export default function LimitPage() {
  const [loading, setLoading] = useState(true);
  const [rules, setRules] = useState<SpeedLimitRule[]>([]);
  const [tunnels, setTunnels] = useState<Tunnel[]>([]);

  const [modalOpen, setModalOpen] = useState(false);
  const [deleteModalOpen, setDeleteModalOpen] = useState(false);
  const [isEdit, setIsEdit] = useState(false);
  const [submitLoading, setSubmitLoading] = useState(false);
  const [deleteLoading, setDeleteLoading] = useState(false);
  const [ruleToDelete, setRuleToDelete] = useState<SpeedLimitRule | null>(null);

  const [form, setForm] = useState<SpeedLimitForm>({
    name: "",
    speed: 100,
    tunnelId: null,
    tunnelName: "",
    status: 1,
  });

  const [errors, setErrors] = useState<{ [key: string]: string }>({});

  useEffect(() => {
    loadData();
  }, []);

  const loadData = async () => {
    setLoading(true);
    try {
      const [rulesRes, tunnelsRes] = await Promise.all([
        getSpeedLimitList(),
        getTunnelList(),
      ]);

      if (rulesRes.code === 0) {
        setRules(rulesRes.data || []);
      } else {
        toast.error(rulesRes.msg || "获取限速规则失败");
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

  const validateForm = (): boolean => {
    const newErrors: { [key: string]: string } = {};

    if (!form.name.trim()) {
      newErrors.name = "请输入规则名称";
    } else if (form.name.length < 2 || form.name.length > 50) {
      newErrors.name = "规则名称长度应在2-50个字符之间";
    }

    if (!form.speed || form.speed < 1) {
      newErrors.speed = "请输入有效的速度限制（≥1 Mbps）";
    }

    if (!form.tunnelId) {
      newErrors.tunnelId = "请选择要绑定的隧道";
    }

    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  };

  const handleAdd = () => {
    setIsEdit(false);
    setForm({ name: "", speed: 100, tunnelId: null, tunnelName: "", status: 1 });
    setErrors({});
    setModalOpen(true);
  };

  const handleEdit = (rule: SpeedLimitRule) => {
    setIsEdit(true);
    setForm({
      id: rule.id,
      name: rule.name,
      speed: rule.speed,
      tunnelId: rule.tunnelId,
      tunnelName: rule.tunnelName,
      status: rule.status,
    });
    setErrors({});
    setModalOpen(true);
  };

  const handleDelete = (rule: SpeedLimitRule) => {
    setRuleToDelete(rule);
    setDeleteModalOpen(true);
  };

  const confirmDelete = async () => {
    if (!ruleToDelete) return;

    setDeleteLoading(true);
    try {
      const res = await deleteSpeedLimit(ruleToDelete.id);
      if (res.code === 0) {
        toast.success("删除成功");
        setDeleteModalOpen(false);
        loadData();
      } else {
        toast.error(res.msg || "删除失败");
      }
    } catch (error) {
      console.error("删除失败:", error);
      toast.error("删除失败");
    } finally {
      setDeleteLoading(false);
    }
  };

  const handleSubmit = async () => {
    if (!validateForm()) return;

    setSubmitLoading(true);
    try {
      let res;
      if (isEdit) {
        res = await updateSpeedLimit(form);
      } else {
        const { id, ...createData } = form;
        void id;
        res = await createSpeedLimit(createData);
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

  if (loading) {
    return (
      <div>
        <PageHeader title="限速管理" desc="按隧道绑定限速规则" />
        <Card>
          <Loading text="正在加载..." />
        </Card>
      </div>
    );
  }

  return (
    <div>
      <PageHeader
        title="限速管理"
        desc={rules.length > 0 ? `共 ${rules.length} 条限速规则` : "按隧道绑定限速规则"}
        actions={
          <Button variant="primary" size="sm" onClick={handleAdd}>
            <Plus size={15} /> 新增
          </Button>
        }
      />

      {rules.length > 0 ? (
        <div className="grid grid-cols-1 gap-4 sm:grid-cols-2 lg:grid-cols-3 xl:grid-cols-4">
          {rules.map((rule) => (
            <Card key={rule.id} hover>
              <CardHead
                title={<span className="truncate">{rule.name}</span>}
                desc={
                  <span className="inline-flex items-center gap-1 font-mono">
                    <Gauge size={12} /> {rule.speed} Mbps
                  </span>
                }
                right={
                  <Badge tone={rule.status === 1 ? "green" : "red"} dot>
                    {rule.status === 1 ? "运行" : "异常"}
                  </Badge>
                }
              />
              <div className="px-5 pb-5">
                <div className="mb-4 flex items-center justify-between gap-2 rounded-xl border border-white/[0.07] bg-white/[0.03] px-3 py-2.5">
                  <span className="text-xs text-slate-400">绑定隧道</span>
                  {rule.tunnelName ? (
                    <Badge tone="cyan">{rule.tunnelName}</Badge>
                  ) : (
                    <span className="text-xs text-slate-500">未绑定</span>
                  )}
                </div>
                <div className="flex gap-2">
                  <Button variant="soft" size="sm" className="flex-1" onClick={() => handleEdit(rule)}>
                    <Pencil size={14} /> 编辑
                  </Button>
                  <Button variant="danger" size="sm" className="flex-1" onClick={() => handleDelete(rule)}>
                    <Trash2 size={14} /> 删除
                  </Button>
                </div>
              </div>
            </Card>
          ))}
        </div>
      ) : (
        <Card>
          <Empty
            text="暂无限速规则，点击右上按钮开始创建"
            action={
              <Button variant="primary" size="sm" onClick={handleAdd}>
                <Plus size={15} /> 新增规则
              </Button>
            }
          />
        </Card>
      )}

      <Modal
        open={modalOpen}
        onClose={() => setModalOpen(false)}
        title={isEdit ? "编辑限速规则" : "新增限速规则"}
        desc={isEdit ? "修改现有限速规则的配置信息" : "创建新的限速规则并绑定到隧道"}
        footer={
          <>
            <Button variant="ghost" onClick={() => setModalOpen(false)}>
              取消
            </Button>
            <Button variant="primary" onClick={handleSubmit} loading={submitLoading}>
              {isEdit ? "保存修改" : "创建规则"}
            </Button>
          </>
        }
      >
        <div className="space-y-4">
          <Field label="规则名称" hint={errors.name}>
            <Input
              placeholder="请输入限速规则名称"
              value={form.name}
              onChange={(e) => setForm((prev) => ({ ...prev, name: e.target.value }))}
            />
          </Field>

          <Field label="速度限制（Mbps）" hint={errors.speed}>
            <Input
              type="number"
              min={1}
              placeholder="请输入速度限制"
              value={form.speed.toString()}
              onChange={(e) =>
                setForm((prev) => ({ ...prev, speed: parseInt(e.target.value) || 0 }))
              }
            />
          </Field>

          <Field
            label="绑定隧道"
            hint={errors.tunnelId || (isEdit ? "编辑时无法修改绑定隧道" : undefined)}
          >
            <Select
              value={form.tunnelId ? String(form.tunnelId) : ""}
              disabled={isEdit}
              onChange={(e) => {
                const v = e.target.value;
                if (v) {
                  const selectedTunnel = tunnels.find((t) => t.id === parseInt(v));
                  setForm((prev) => ({
                    ...prev,
                    tunnelId: parseInt(v),
                    tunnelName: selectedTunnel?.name || "",
                  }));
                } else {
                  setForm((prev) => ({ ...prev, tunnelId: null, tunnelName: "" }));
                }
              }}
            >
              <option value="">请选择要绑定的隧道</option>
              {tunnels.map((tunnel) => (
                <option key={tunnel.id} value={String(tunnel.id)}>
                  {tunnel.name}
                </option>
              ))}
            </Select>
          </Field>
        </div>
      </Modal>

      <Modal
        open={deleteModalOpen}
        onClose={() => setDeleteModalOpen(false)}
        title="确认删除"
        desc="此操作无法撤销，删除后该规则将永久消失"
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
        <p className="text-sm text-slate-300">
          确定要删除限速规则 <span className="font-semibold text-white">"{ruleToDelete?.name}"</span> 吗？
        </p>
      </Modal>
    </div>
  );
}
