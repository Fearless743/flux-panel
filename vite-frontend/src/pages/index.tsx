import { useState, useEffect, useRef } from "react";
import { useNavigate } from "react-router-dom";
import toast from 'react-hot-toast';
import axios from 'axios';
import { User, LockKeyhole, LogIn, ShieldCheck, Zap, Gauge, Globe } from "lucide-react";
import { isWebViewFunc } from '@/utils/panel';
import { siteConfig } from '@/config/site';
import DefaultLayout from "@/layouts/default";
import { Button, Card, Field, Input } from "@/components/ui";
import { login, LoginData, checkCaptcha } from "@/api";
import "@/utils/tac.css";
import "@/utils/tac.min.js";
import bgImage from "@/images/bg.jpg";


interface LoginForm {
  username: string;
  password: string;
  captchaId: string;
}



interface CaptchaConfig {
  requestCaptchaDataUrl: string;
  validCaptchaUrl: string;
  bindEl: string;
  validSuccess: (res: any, captcha: any, tac: any) => void;
  validFail?: (res: any, captcha: any, tac: any) => void;
  btnCloseFun?: (event: any, tac: any) => void;
  btnRefreshFun?: (event: any, tac: any) => void;
}

interface CaptchaStyle {
  btnUrl?: string;
  bgUrl?: string;
  logoUrl?: string | null;
  moveTrackMaskBgColor?: string;
  moveTrackMaskBorderColor?: string;
}

export default function IndexPage() {
  const [form, setForm] = useState<LoginForm>({
    username: "",
    password: "",
    captchaId: "",
  });
  const [loading, setLoading] = useState(false);
  const [errors, setErrors] = useState<Partial<LoginForm>>({});
  const [showCaptcha, setShowCaptcha] = useState(false);
  const navigate = useNavigate();
  const tacInstanceRef = useRef<any>(null);
  const captchaContainerRef = useRef<HTMLDivElement>(null);
  const [isWebView, setIsWebView] = useState(false);
  // 清理验证码实例
  useEffect(() => {
    return () => {
      if (tacInstanceRef.current) {
        tacInstanceRef.current.destroyWindow();
        tacInstanceRef.current = null;
      }
    };
  }, []);
  // 检测是否在WebView中运行
  useEffect(() => {
    setIsWebView(isWebViewFunc());
  }, []);
  // 验证表单
  const validateForm = (): boolean => {
    const newErrors: Partial<LoginForm> = {};

    if (!form.username.trim()) {
      newErrors.username = '请输入用户名';
    }

    if (!form.password.trim()) {
      newErrors.password = '请输入密码';
    } else if (form.password.length < 6) {
      newErrors.password = '密码长度至少6位';
    }


    setErrors(newErrors);
    return Object.keys(newErrors).length === 0;
  };

  // 处理输入变化
  const handleInputChange = (field: keyof LoginForm, value: string) => {
    setForm(prev => ({ ...prev, [field]: value }));
    // 清除该字段的错误
    if (errors[field]) {
      setErrors(prev => ({ ...prev, [field]: undefined }));
    }
  };

  // 初始化验证码
  const initCaptcha = async () => {
    if (!window.TAC || !captchaContainerRef.current) {
      return;
    }

    try {
      // 清理之前的验证码实例
      if (tacInstanceRef.current) {
        tacInstanceRef.current.destroyWindow();
        tacInstanceRef.current = null;
      }

      // 使用axios的baseURL，确保在WebView中使用正确的面板地址
      const baseURL = axios.defaults.baseURL || (import.meta.env.VITE_API_BASE ? `${import.meta.env.VITE_API_BASE}/api/v1/` : '/api/v1/');

      const config: CaptchaConfig = {
        requestCaptchaDataUrl: `${baseURL}captcha/generate`,
        validCaptchaUrl: `${baseURL}captcha/verify`,
        bindEl: "#captcha-container",
        validSuccess: (res: any, _: any, tac: any) => {


          form.captchaId = res.data.validToken

          setShowCaptcha(false);
          tac.destroyWindow();
          performLogin();
        },
        validFail: (_: any, _captcha: any, tac: any) => {
          tac.reloadCaptcha();
        },
        btnCloseFun: (_event: any, tac: any) => {
          setShowCaptcha(false);
          tac.destroyWindow();
          setLoading(false);
        },
        btnRefreshFun: (_event: any, tac: any) => {
          tac.reloadCaptcha();
        }
      };

      // 检测暗黑模式
      const isDarkMode = document.documentElement.classList.contains('dark') ||
                        document.documentElement.getAttribute('data-theme') === 'dark' ||
                        window.matchMedia('(prefers-color-scheme: dark)').matches;

      // 根据主题调整颜色
      const trackColor = isDarkMode ? "#4a5568" : "#7db0be"; // 暗黑模式使用更深的灰蓝色

      const style: CaptchaStyle = {
        bgUrl: bgImage,
        logoUrl: null,
        moveTrackMaskBgColor: trackColor,
        moveTrackMaskBorderColor: trackColor
      };

      tacInstanceRef.current = new window.TAC(config, style);
      tacInstanceRef.current.init();

    } catch (error) {
      console.error('初始化验证码失败:', error);
      toast.error('验证码初始化失败，请刷新页面重试');
      setShowCaptcha(false);
      setLoading(false);
    }
  };

  // 执行登录请求
  const performLogin = async () => {


    try {
      const loginData: LoginData = {
        username: form.username.trim(),
        password: form.password,
        captchaId: form.captchaId,
      };

      const response = await login(loginData);

      if (response.code !== 0) {
        toast.error(response.msg || "登录失败");
        return;
      }

      // 检查是否需要强制修改密码
      if (response.data.requirePasswordChange) {
        localStorage.setItem('token', response.data.token);
        localStorage.setItem("role_id", response.data.role_id.toString());
        localStorage.setItem("name", response.data.name);
        localStorage.setItem("admin", (response.data.role_id === 0).toString());
        toast.success('检测到默认密码，即将跳转到修改密码页面');
        navigate("/change-password");
        return;
      }

      // 保存登录信息
      localStorage.setItem('token', response.data.token);
      localStorage.setItem("role_id", response.data.role_id.toString());
      localStorage.setItem("name", response.data.name);
      localStorage.setItem("admin", (response.data.role_id === 0).toString());

      // 登录成功
      toast.success('登录成功');
      navigate("/dashboard");

    } catch (error) {
      console.error('登录错误:', error);
      toast.error("网络错误，请稍后重试");
    } finally {
      setLoading(false);
    }
  };

  const handleLogin = async () => {
    if (!validateForm()) return;

    setLoading(true);

    try {
      // 先检查是否需要验证码
      const checkResponse = await checkCaptcha();

      if (checkResponse.code !== 0) {
        toast.error("检查验证码状态失败，请重试" + checkResponse.msg);
        setLoading(false);
        return;
      }

      // 根据返回值决定是否显示验证码
      if (checkResponse.data === 0) {
        // 不需要验证码，直接登录
        await performLogin();
      } else {
        // 需要验证码，显示验证码弹层
        setShowCaptcha(true);
        // 延时初始化验证码，确保DOM已渲染
        setTimeout(() => {
          initCaptcha();
        }, 100);
      }
    } catch (error) {
      console.error('检查验证码状态错误:', error);
      toast.error("网络错误，请稍后重试" + error);
      setLoading(false);
    }
  };


  const handleKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter' && !loading) {
      handleLogin();
    }
  };

  const features = [
    { icon: <Gauge size={15} />, title: "实时转发", desc: "端口转发状态实时可视" },
    { icon: <Globe size={15} />, title: "内网穿透", desc: "多节点隧道统一管理" },
    { icon: <ShieldCheck size={15} />, title: "安全可靠", desc: "滑动验证码登录保护" },
  ];

  return (
    <DefaultLayout>
      <section className="flex min-h-[calc(100dvh-120px)] items-center justify-center py-8 sm:min-h-[calc(100dvh-160px)]">
        <div className="grid w-full max-w-4xl items-stretch gap-5 lg:grid-cols-2">
          {/* 左侧品牌展示（大屏） */}
          <div className="card relative hidden overflow-hidden p-8 lg:flex lg:flex-col lg:justify-between">
            <div className="relative">
              <div className="flex h-12 w-12 items-center justify-center rounded-2xl bg-blue-600">
                <Zap size={22} className="text-white" />
              </div>
              <h1 className="text-gradient mt-5 text-t3xl font-bold leading-tight tracking-tight">
                {siteConfig.name}
              </h1>
              <p className="mt-2 text-tbase leading-relaxed text-slate-400">
                高性能端口转发与内网穿透管理面板，一个地址掌控所有节点与隧道。
              </p>
            </div>
            <div className="relative mt-8 space-y-3">
              {features.map((f) => (
                <div key={f.title} className="flex items-center gap-3 rounded-xl border border-white/[0.07] bg-white/[0.03] px-3.5 py-3">
                  <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-blue-400/20 bg-blue-400/10 text-blue-300">
                    {f.icon}
                  </div>
                  <div className="min-w-0">
                    <p className="text-tsm font-semibold text-white">{f.title}</p>
                    <p className="truncate text-txs text-slate-400">{f.desc}</p>
                  </div>
                </div>
              ))}
            </div>
          </div>

          {/* 右侧登录卡 */}
          <Card className="relative flex flex-col justify-center overflow-hidden p-6 sm:p-8">
            <div className="relative">
              <div className="mb-6 flex h-11 w-11 items-center justify-center rounded-2xl border border-blue-400/20 bg-blue-400/10 text-blue-300 lg:hidden">
                <Zap size={20} />
              </div>
              <h2 className="text-gradient text-t2xl font-bold tracking-tight">欢迎回来</h2>
              <p className="mt-1 text-tsm text-slate-400">请输入您的账号信息登录面板</p>

              <div className="mt-6 flex flex-col gap-4">
                <Field label="用户名">
                  <div className="relative">
                    <User size={15} className="pointer-events-none absolute left-3.5 top-1/2 -translate-y-1/2 text-slate-500" />
                    <Input
                      placeholder="请输入用户名"
                      value={form.username}
                      onChange={(e) => handleInputChange('username', e.target.value)}
                      onKeyDown={handleKeyPress}
                      disabled={loading}
                      className="!pl-10"
                    />
                  </div>
                  {errors.username && <p className="mt-1 text-txs text-red-400">{errors.username}</p>}
                </Field>

                <Field label="密码">
                  <div className="relative">
                    <LockKeyhole size={15} className="pointer-events-none absolute left-3.5 top-1/2 -translate-y-1/2 text-slate-500" />
                    <Input
                      placeholder="请输入密码"
                      type="password"
                      value={form.password}
                      onChange={(e) => handleInputChange('password', e.target.value)}
                      onKeyDown={handleKeyPress}
                      disabled={loading}
                      className="!pl-10"
                    />
                  </div>
                  {errors.password && <p className="mt-1 text-txs text-red-400">{errors.password}</p>}
                </Field>

                <Button
                  variant="primary"
                  size="lg"
                  onClick={handleLogin}
                  loading={loading}
                  disabled={loading}
                  className="mt-1 w-full"
                >
                  {!loading && <LogIn size={16} />}
                  {loading ? (showCaptcha ? "验证中..." : "登录中...") : "登 录"}
                </Button>
              </div>
            </div>
          </Card>
        </div>

        {/* 版权信息 - 固定在底部，不占据布局空间 */}
        <div className="pointer-events-none fixed inset-x-0 bottom-4 py-4 text-center">
          <p className="pointer-events-auto text-xs text-slate-500">
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
          <p className="mono mt-1 text-xs text-slate-500">
            v{isWebView ? siteConfig.app_version : siteConfig.version}
          </p>
        </div>

        {/* 验证码弹层 */}
        {showCaptcha && (
          <div className="fixed inset-0 z-50 flex items-center justify-center">
            <div className="absolute inset-0 bg-black/60 backdrop-blur-sm captcha-backdrop-enter" />
            <div className="relative z-10 mb-4">
              <div
                id="captcha-container"
                ref={captchaContainerRef}
                className="flex w-full justify-center"
                style={{
                  filter: document.documentElement.classList.contains('dark') ||
                          document.documentElement.getAttribute('data-theme') === 'dark' ||
                          window.matchMedia('(prefers-color-scheme: dark)').matches
                          ? 'brightness(0.8) contrast(0.9)' : 'none'
                }}
              />
            </div>
          </div>
        )}
      </section>
    </DefaultLayout>
  );
}
