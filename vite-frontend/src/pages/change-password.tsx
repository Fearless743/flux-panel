import { useState } from "react";
import { useNavigate } from "react-router-dom";
import toast from 'react-hot-toast';
import { ShieldAlert, KeyRound, User, LockKeyhole, CheckCheck } from "lucide-react";

import { updatePassword } from "@/api";
import DefaultLayout from "@/layouts/default";
import { safeLogout } from "@/utils/logout";
import { Button, Card, Field, Input } from "@/components/ui";

interface PasswordForm {
  newUsername: string;
  currentPassword: string;
  newPassword: string;
  confirmPassword: string;
}

export default function ChangePasswordPage() {
  const [form, setForm] = useState<PasswordForm>({
    newUsername: '',
    currentPassword: '',
    newPassword: '',
    confirmPassword: ''
  });
  const [loading, setLoading] = useState(false);
  const [errors, setErrors] = useState<Partial<PasswordForm>>({});
  const navigate = useNavigate();

  const validateForm = (): boolean => {
    const newErrors: Partial<PasswordForm> = {};

    if (!form.newUsername.trim()) {
      newErrors.newUsername = '请输入新用户名';
    } else if (form.newUsername.length < 3) {
      newErrors.newUsername = '用户名长度至少3位';
    } else if (form.newUsername.length > 20) {
      newErrors.newUsername = '用户名长度不能超过20位';
    }

    if (!form.currentPassword.trim()) {
      newErrors.currentPassword = '请输入当前密码';
    }

    if (!form.newPassword.trim()) {
      newErrors.newPassword = '请输入新密码';
    } else if (form.newPassword.length < 6) {
      newErrors.newPassword = '新密码长度不能少于6位';
    } else if (form.newPassword.length > 20) {
      newErrors.newPassword = '新密码长度不能超过20位';
    }

    if (!form.confirmPassword.trim()) {
      newErrors.confirmPassword = '请再次输入新密码';
    } else if (form.confirmPassword !== form.newPassword) {
      newErrors.confirmPassword = '两次输入密码不一致';
    }

    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  };

  const handleInputChange = (field: keyof PasswordForm, value: string) => {
    setForm(prev => ({ ...prev, [field]: value }));
    if (errors[field]) {
      setErrors(prev => ({ ...prev, [field]: undefined }));
    }
  };

  const handleSubmit = async () => {
    if (!validateForm()) return;

    setLoading(true);
    try {
      const response = await updatePassword(form);

      if (response.code === 0) {
        toast.success(response.msg || '账号密码修改成功');

        // 使用 toast 确认对话框的替代方案
        setTimeout(() => {
          toast.success('即将跳转到登陆页面，请重新登录');
          setTimeout(() => {
            logout();
          }, 1000);
        }, 1000);
      } else {
        toast.error(response.msg || '账号密码修改失败');
      }
    } catch (error) {
      console.error('修改账号密码错误:', error);
      toast.error('修改账号密码时发生错误');
    } finally {
      setLoading(false);
    }
  };

  const logout = () => {
    safeLogout();
    navigate('/');
  };

  const handleKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !loading) {
      handleSubmit();
    }
  };

  const fields: { key: keyof PasswordForm; label: string; placeholder: string; icon: React.ReactNode }[] = [
    { key: 'newUsername', label: '新用户名', placeholder: '请输入新用户名（3-20 位）', icon: <User size={15} /> },
    { key: 'currentPassword', label: '当前密码', placeholder: '请输入当前密码', icon: <LockKeyhole size={15} /> },
    { key: 'newPassword', label: '新密码', placeholder: '请输入新密码（6-20 位）', icon: <KeyRound size={15} /> },
    { key: 'confirmPassword', label: '确认新密码', placeholder: '请再次输入新密码', icon: <CheckCheck size={15} /> },
  ];

  return (
    <DefaultLayout>
      <section className="flex min-h-[calc(100dvh-160px)] items-center justify-center py-8">
        <Card className="relative w-full max-w-lg overflow-hidden p-6 sm:p-8">
          <div className="relative">
            <div className="flex flex-col items-center text-center">
              <div className="mb-4 flex h-12 w-12 items-center justify-center rounded-2xl border border-amber-400/25 bg-amber-400/10 text-amber-300">
                <ShieldAlert size={22} />
              </div>
              <h1 className="text-gradient text-[1.35rem] font-bold tracking-tight">安全提醒</h1>
              <p className="mt-1.5 max-w-sm text-[0.8rem] leading-relaxed text-slate-400">
                检测到您使用的是默认账号密码，为了您的账户安全，请立即修改
              </p>
            </div>

            <div className="mt-6 flex flex-col gap-4">
              {fields.map((f) => (
                <Field key={f.key} label={f.label}>
                  <div className="relative">
                    <span className="pointer-events-none absolute left-3.5 top-1/2 -translate-y-1/2 text-slate-500">
                      {f.icon}
                    </span>
                    <Input
                      placeholder={f.placeholder}
                      type={f.key === 'newUsername' ? 'text' : 'password'}
                      value={form[f.key]}
                      onChange={(e) => handleInputChange(f.key, e.target.value)}
                      onKeyDown={handleKeyPress}
                      disabled={loading}
                      className="!pl-10"
                    />
                  </div>
                  {errors[f.key] && <p className="mt-1 text-[0.72rem] text-red-400">{errors[f.key]}</p>}
                </Field>
              ))}

              <Button
                variant="primary"
                size="lg"
                onClick={handleSubmit}
                loading={loading}
                disabled={loading}
                className="mt-1 w-full"
              >
                {!loading && <KeyRound size={16} />}
                {loading ? "修改中..." : "立即修改账号密码"}
              </Button>

              <div className="rounded-xl border border-amber-400/20 bg-amber-400/[0.07] px-3.5 py-2.5 text-center text-[0.78rem] text-amber-300">
                注意：修改账号密码后需要重新登录
              </div>
            </div>
          </div>
        </Card>
      </section>
    </DefaultLayout>
  );
}
