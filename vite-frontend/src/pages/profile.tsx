import { useState, useEffect } from 'react';
import { toast } from 'react-hot-toast';
import { useNavigate } from 'react-router-dom';
import {
  UserRound, Gauge, Users, Settings2, KeyRound, LogOut,
  User as UserIcon, LockKeyhole, CheckCheck,
} from "lucide-react";
import { isWebViewFunc } from '@/utils/panel';
import { siteConfig } from '@/config/site';
import { updatePassword } from '@/api';
import { safeLogout } from '@/utils/logout';
import { Button, Card, Badge, Modal, Field, Input, PageHeader } from "@/components/ui";

interface PasswordForm {
  newUsername: string;
  currentPassword: string;
  newPassword: string;
  confirmPassword: string;
}


interface MenuItem {
  path: string;
  label: string;
  icon: React.ReactNode;
  description: string;
}

export default function ProfilePage() {
  const navigate = useNavigate();
  const [pwdOpen, setPwdOpen] = useState(false);
  const [username, setUsername] = useState('');
  const [isAdmin, setIsAdmin] = useState(false);
  const [passwordLoading, setPasswordLoading] = useState(false);
  const [passwordForm, setPasswordForm] = useState<PasswordForm>({
    newUsername: '',
    currentPassword: '',
    newPassword: '',
    confirmPassword: ''
  });

  useEffect(() => {
    // 获取用户信息
    const name = localStorage.getItem('name') || 'Admin';

    // 兼容处理：如果没有admin字段，根据role_id判断（0为管理员）
    let adminFlag = localStorage.getItem('admin') === 'true';
    if (localStorage.getItem('admin') === null) {
      const roleId = parseInt(localStorage.getItem('role_id') || '1', 10);
      adminFlag = roleId === 0;
      // 补充设置admin字段，避免下次再次判断
      localStorage.setItem('admin', adminFlag.toString());
    }

    setUsername(name);
    setIsAdmin(adminFlag);
  }, []);

  // 管理员菜单项
  const adminMenuItems: MenuItem[] = [
    {
      path: '/limit',
      label: '限速管理',
      icon: <Gauge size={19} />,
      description: '管理用户限速策略'
    },
    {
      path: '/user',
      label: '用户管理',
      icon: <Users size={19} />,
      description: '管理系统用户'
    },
    {
      path: '/config',
      label: '网站配置',
      icon: <Settings2 size={19} />,
      description: '配置网站设置'
    }
  ];

  // 退出登录
  const handleLogout = () => {
    safeLogout();
    navigate('/', { replace: true });
  };

  // 密码表单验证
  const validatePasswordForm = (): boolean => {
    if (!passwordForm.newUsername.trim()) {
      toast.error('请输入新用户名');
      return false;
    }
    if (passwordForm.newUsername.length < 3) {
      toast.error('用户名长度至少3位');
      return false;
    }
    if (!passwordForm.currentPassword) {
      toast.error('请输入当前密码');
      return false;
    }
    if (!passwordForm.newPassword) {
      toast.error('请输入新密码');
      return false;
    }
    if (passwordForm.newPassword.length < 6) {
      toast.error('新密码长度不能少于6位');
      return false;
    }
    if (passwordForm.newPassword !== passwordForm.confirmPassword) {
      toast.error('两次输入密码不一致');
      return false;
    }
    return true;
  };

  // 提交密码修改
  const handlePasswordSubmit = async () => {
    if (!validatePasswordForm()) return;

    setPasswordLoading(true);
    try {
      const response = await updatePassword(passwordForm);
      if (response.code === 0) {
        toast.success('密码修改成功，请重新登录');
        setPwdOpen(false);
        handleLogout();
      } else {
        toast.error(response.msg || '密码修改失败');
      }
    } catch (error) {
      toast.error('修改密码时发生错误');
      console.error('修改密码错误:', error);
    } finally {
      setPasswordLoading(false);
    }
  };

  // 重置密码表单
  const resetPasswordForm = () => {
    setPasswordForm({
      newUsername: '',
      currentPassword: '',
      newPassword: '',
      confirmPassword: ''
    });
  };

  const pwdFields: { key: keyof PasswordForm; label: string; placeholder: string; icon: React.ReactNode }[] = [
    { key: 'newUsername', label: '新用户名', placeholder: '请输入新用户名（至少3位）', icon: <UserIcon size={15} /> },
    { key: 'currentPassword', label: '当前密码', placeholder: '请输入当前密码', icon: <LockKeyhole size={15} /> },
    { key: 'newPassword', label: '新密码', placeholder: '请输入新密码（至少6位）', icon: <KeyRound size={15} /> },
    { key: 'confirmPassword', label: '确认密码', placeholder: '请再次输入新密码', icon: <CheckCheck size={15} /> },
  ];

  const gridBtn =
    "group flex flex-col items-center gap-2 rounded-2xl border border-white/[0.07] bg-white/[0.03] p-4 transition-all hover:border-blue-400/25 hover:bg-blue-400/[0.06] active:scale-[0.97]";

  return (
    <div className="mx-auto w-full max-w-2xl">
      <PageHeader title="个人中心" desc="管理账号信息与快捷功能入口" />

      <div className="space-y-5 pb-24">
        {/* 用户信息卡片 */}
        <Card className="relative overflow-hidden p-5">
          <div className="relative flex items-center gap-4">
            <div className="flex h-14 w-14 shrink-0 items-center justify-center rounded-2xl bg-blue-600">
              <UserRound size={26} className="text-white" />
            </div>
            <div className="min-w-0 flex-1">
              <h3 className="truncate text-[1.05rem] font-bold tracking-tight text-white">{username}</h3>
              <div className="mt-1.5 flex flex-wrap items-center gap-2">
                <Badge tone={isAdmin ? "cyan" : "slate"} dot>
                  {isAdmin ? '管理员' : '普通用户'}
                </Badge>
                <span className="text-[0.72rem] text-slate-500">
                  {new Date().toLocaleDateString('zh-CN')}
                </span>
              </div>
            </div>
          </div>
        </Card>

        {/* 功能九宫格 */}
        <Card className="p-4">
          <div className="grid grid-cols-3 gap-3">
            {/* 管理员功能 */}
            {isAdmin && adminMenuItems.map((item) => (
              <button
                key={item.path}
                onClick={() => navigate(item.path)}
                title={item.description}
                className={gridBtn}
              >
                <div className="flex h-11 w-11 items-center justify-center rounded-xl border border-blue-400/20 bg-blue-400/10 text-blue-300 transition-transform group-active:scale-95">
                  {item.icon}
                </div>
                <span className="text-center text-[0.76rem] font-medium text-slate-200">{item.label}</span>
              </button>
            ))}

            {/* 修改密码 */}
            <button
              onClick={() => setPwdOpen(true)}
              className={gridBtn}
            >
              <div className="flex h-11 w-11 items-center justify-center rounded-xl border border-blue-400/20 bg-blue-400/10 text-blue-300 transition-transform group-active:scale-95">
                <KeyRound size={19} />
              </div>
              <span className="text-center text-[0.76rem] font-medium text-slate-200">修改密码</span>
            </button>

            {/* 退出登录 */}
            <button
              onClick={handleLogout}
              className={gridBtn}
            >
              <div className="flex h-11 w-11 items-center justify-center rounded-xl border border-red-400/20 bg-red-400/10 text-red-300 transition-transform group-active:scale-95">
                <LogOut size={19} />
              </div>
              <span className="text-center text-[0.76rem] font-medium text-slate-200">退出登录</span>
            </button>
          </div>
        </Card>

        <div className="py-2 text-center">
          <p className="text-xs text-slate-500">
            Powered by{' '}
            <a
              href="https://github.com/bqlpfy/flux-panel"
              target="_blank"
              rel="noopener noreferrer"
              className="text-slate-400 transition-colors hover:text-blue-300"
            >
              flux-panel
            </a>
          </p>
          <p className="mono mt-1 text-xs text-slate-600">
            v{isWebViewFunc() ? siteConfig.app_version : siteConfig.version}
          </p>
        </div>
      </div>


      {/* 修改密码弹窗 */}
      <Modal
        open={pwdOpen}
        onClose={() => {
          setPwdOpen(false);
          resetPasswordForm();
        }}
        title="修改密码"
        desc="修改后将自动退出，需重新登录"
        footer={
          <>
            <Button
              variant="ghost"
              onClick={() => {
                setPwdOpen(false);
                resetPasswordForm();
              }}
            >
              取消
            </Button>
            <Button
              variant="primary"
              onClick={handlePasswordSubmit}
              loading={passwordLoading}
              disabled={passwordLoading}
            >
              确定
            </Button>
          </>
        }
      >
        <div className="space-y-4">
          {pwdFields.map((f) => (
            <Field key={f.key} label={f.label}>
              <div className="relative">
                <span className="pointer-events-none absolute left-3.5 top-1/2 -translate-y-1/2 text-slate-500">
                  {f.icon}
                </span>
                <Input
                  type={f.key === 'newUsername' ? 'text' : 'password'}
                  placeholder={f.placeholder}
                  value={passwordForm[f.key]}
                  onChange={(e: React.ChangeEvent<HTMLInputElement>) => setPasswordForm(prev => ({ ...prev, [f.key]: e.target.value }))}
                  className="!pl-10"
                />
              </div>
            </Field>
          ))}
        </div>
      </Modal>
    </div>
  );
}
