import { useState, useEffect } from "react";
import toast from 'react-hot-toast';
import { AreaChart, Area, XAxis, YAxis, CartesianGrid, Tooltip, ResponsiveContainer } from 'recharts';
import {
  Database, Activity, Network, Link2, RefreshCw, Copy, Check,
  Clock3, Server, ArrowDown, CalendarClock,
} from "lucide-react";

import { Button, Card, CardHead, Badge, Modal, PageHeader, Empty, Loading, Stat, Progress } from "@/components/ui";
import { getUserPackageInfo } from "@/api";

interface UserInfo {
  flow: number;
  inFlow: number;
  outFlow: number;
  num: number;
  expTime?: string;
  flowResetTime?: number;
}

interface UserTunnel {
  id: number;
  tunnelId: number;
  tunnelName: string;
  flow: number;
  inFlow: number;
  outFlow: number;
  num: number;
  expTime?: string;
  flowResetTime?: number;
  tunnelFlow: number;
}

interface Forward {
  id: number;
  name: string;
  tunnelId: number;
  tunnelName: string;
  inIp: string;
  inPort: number;
  remoteAddr: string;
  inFlow: number;
  outFlow: number;
}

interface AddressItem {
  id: number;
  ip: string;
  address: string;
  copying: boolean;
}

interface StatisticsFlow {
  id: number;
  userId: number;
  flow: number;
  totalFlow: number;
  time: string;
}

function GradientNum({ children }: { children: React.ReactNode }) {
  return <span className="text-slate-50">{children}</span>;
}

export default function DashboardPage() {
  const [loading, setLoading] = useState(true);
  const [refreshing, setRefreshing] = useState(false);
  const [userInfo, setUserInfo] = useState<UserInfo>({} as UserInfo);
  const [userTunnels, setUserTunnels] = useState<UserTunnel[]>([]);
  const [forwardList, setForwardList] = useState<Forward[]>([]);
  const [statisticsFlows, setStatisticsFlows] = useState<StatisticsFlow[]>([]);
  const [isAdmin, setIsAdmin] = useState(false);

  const [addressModalOpen, setAddressModalOpen] = useState(false);
  const [addressModalTitle, setAddressModalTitle] = useState('');
  const [addressList, setAddressList] = useState<AddressItem[]>([]);

  // 检查有效期通知
  const checkExpirationNotifications = (userInfo: UserInfo, tunnels: UserTunnel[]) => {
    // 避免重复通知，检查是否已经显示过
    const notificationKey = `expiration-${userInfo.expTime}-${tunnels.map(t => t.expTime).join(',')}`;
    const lastNotified = localStorage.getItem('lastNotified');

    if (lastNotified === notificationKey) {
      return; // 已经通知过，不重复显示
    }

    let hasNotification = false;

    // 检查主账户有效期
    if (userInfo.expTime) {
      const expDate = new Date(userInfo.expTime);
      const now = new Date();

      if (!isNaN(expDate.getTime()) && expDate > now) {
        const diffTime = expDate.getTime() - now.getTime();
        const diffDays = Math.ceil(diffTime / (1000 * 60 * 60 * 24));

        if (diffDays <= 7 && diffDays > 0) {
          hasNotification = true;
          if (diffDays === 1) {
            toast('账户将于明天过期，请及时续费', {
              icon: '⚠️',
              duration: 6000,
            });
          } else {
            toast(`账户将于${diffDays}天后过期，请及时续费`, {
              icon: '⚠️',
              duration: 6000,
            });
          }
        } else if (diffDays <= 0) {
          hasNotification = true;
          toast('账户已过期，请立即续费', {
            icon: '⚠️',
            duration: 8000,
          });
        }
      }
    }

    // 检查隧道有效期
    tunnels.forEach(tunnel => {
      if (tunnel.expTime) {
        const expDate = new Date(tunnel.expTime);
        const now = new Date();

        if (!isNaN(expDate.getTime()) && expDate > now) {
          const diffTime = expDate.getTime() - now.getTime();
          const diffDays = Math.ceil(diffTime / (1000 * 60 * 60 * 24));

          if (diffDays <= 7 && diffDays > 0) {
            hasNotification = true;
            if (diffDays === 1) {
              toast(`隧道"${tunnel.tunnelName}"将于明天过期`, {
                icon: '⚠️',
                duration: 5000,
              });
            } else {
              toast(`隧道"${tunnel.tunnelName}"将于${diffDays}天后过期`, {
                icon: '⚠️',
                duration: 5000,
              });
            }
          } else if (diffDays <= 0) {
            hasNotification = true;
            toast(`隧道"${tunnel.tunnelName}"已过期`, {
              icon: '⚠️',
              duration: 6000,
            });
          }
        }
      }
    });

    // 如果显示了通知，记录防止重复
    if (hasNotification) {
      localStorage.setItem('lastNotified', notificationKey);
    }
  };

  useEffect(() => {
    // 重置状态并加载数据，防止页面切换时显示旧数据
    setLoading(true);
    setUserInfo({} as UserInfo);
    setUserTunnels([]);
    setForwardList([]);
    setStatisticsFlows([]);

    // 检查用户是否是管理员
    const adminStatus = localStorage.getItem('admin');
    setIsAdmin(adminStatus === 'true');

    loadPackageData();
    localStorage.setItem('e', '/dashboard');
  }, []);

  const loadPackageData = async () => {
    setLoading(true);
    try {
      const res = await getUserPackageInfo();
      if (res.code === 0) {
        const data = res.data;
        setUserInfo(data.userInfo || {});
        setUserTunnels(data.tunnelPermissions || []);
        setForwardList(data.forwards || []);
        setStatisticsFlows(data.statisticsFlows || []);

        // 检查有效期并显示通知
        checkExpirationNotifications(data.userInfo, data.tunnelPermissions || []);
      } else {
        toast.error(res.msg || '获取套餐信息失败');
      }
    } catch (error) {
      console.error('获取套餐信息失败:', error);
      toast.error('获取套餐信息失败');
    } finally {
      setLoading(false);
      setRefreshing(false);
    }
  };

  const handleRefresh = () => {
    setRefreshing(true);
    loadPackageData();
  };

  const formatFlow = (value: number, unit: string = 'bytes'): string => {
    // 99999 表示无限制
    if (value === 99999) {
      return '无限制';
    }

    if (unit === 'gb') {
      return value + ' GB';
    } else {
      if (!value || value <= 0) return '0 B';
      if (value < 1024) return value + ' B';
      const k = 1024;
      const sizes = ['B', 'KB', 'MB', 'GB', 'TB', 'PB', 'EB'];
      const i = Math.min(Math.floor(Math.log(value) / Math.log(k)), sizes.length - 1);
      return (value / Math.pow(k, i)).toFixed(2) + ' ' + sizes[i];
    }
  };

  const formatNumber = (value: number): string => {
    // 99999 表示无限制
    if (value === 99999) {
      return '无限制';
    }
    return value.toString();
  };

  // 处理24小时流量统计数据
  const processFlowChartData = () => {
    // 生成最近24小时的时间数组（从当前小时往前推24小时）
    const now = new Date();
    const hours: string[] = [];
    for (let i = 23; i >= 0; i--) {
      const time = new Date(now.getTime() - i * 60 * 60 * 1000);
      const hourString = time.getHours().toString().padStart(2, '0') + ':00';
      hours.push(hourString);
    }

    // 创建数据映射
    const flowMap = new Map<string, number>();
    statisticsFlows.forEach(item => {
      flowMap.set(item.time, item.flow || 0);
    });

    // 生成图表数据，没有数据的小时显示为0
    return hours.map(hour => ({
      time: hour,
      flow: flowMap.get(hour) || 0,
      // 格式化显示用的流量值
      formattedFlow: formatFlow(flowMap.get(hour) || 0)
    }));
  };


  const getExpStatus = (expTime?: string): { tone: "green" | "slate" | "red" | "amber"; text: string } => {
    if (!expTime) return { tone: 'green', text: '永久' };

    const now = new Date();
    const expDate = new Date(expTime);

    if (isNaN(expDate.getTime())) {
      return { tone: 'slate', text: '无效' };
    }

    if (expDate < now) {
      return { tone: 'red', text: '已过期' };
    }

    const diffTime = expDate.getTime() - now.getTime();
    const diffDays = Math.ceil(diffTime / (1000 * 60 * 60 * 24));

    if (diffDays <= 7) {
      return { tone: 'red', text: `${diffDays}天后过期` };
    } else if (diffDays <= 30) {
      return { tone: 'amber', text: `${diffDays}天后过期` };
    } else {
      return { tone: 'green', text: `${diffDays}天后过期` };
    }
  };

  const calculateUserTotalUsedFlow = (): number => {
    // 后端已按计费类型处理流量，前端直接使用入站+出站总和
    return (userInfo.inFlow || 0) + (userInfo.outFlow || 0);
  };

  const calculateUsagePercentage = (type: 'flow' | 'forwards'): number => {
    if (type === 'flow') {
      const totalUsed = calculateUserTotalUsedFlow();
      const totalLimit = (userInfo.flow || 0) * 1024 * 1024 * 1024;
      // 无限制时返回0%
      if (userInfo.flow === 99999) return 0;
      return totalLimit > 0 ? Math.min((totalUsed / totalLimit) * 100, 100) : 0;
    } else if (type === 'forwards') {
      const totalUsed = forwardList.length;
      const totalLimit = userInfo.num || 0;
      // 无限制时返回0%
      if (userInfo.num === 99999) return 0;
      return totalLimit > 0 ? Math.min((totalUsed / totalLimit) * 100, 100) : 0;
    }
    return 0;
  };

  const calculateTunnelUsedFlow = (tunnel: UserTunnel): number => {
    if (!tunnel) return 0;
    const inFlow = tunnel.inFlow || 0;
    const outFlow = tunnel.outFlow || 0;
    // 后端已按计费类型处理流量，前端直接使用入站+出站总和
    return inFlow + outFlow;
  };

  const calculateTunnelFlowPercentage = (tunnel: UserTunnel): number => {
    const totalUsed = calculateTunnelUsedFlow(tunnel);
    const totalLimit = (tunnel.flow || 0) * 1024 * 1024 * 1024;
    // 无限制时返回0%
    if (tunnel.flow === 99999) return 0;
    return totalLimit > 0 ? Math.min((totalUsed / totalLimit) * 100, 100) : 0;
  };

  const getTunnelUsedForwards = (tunnelId: number): number => {
    return forwardList.filter(forward => forward.tunnelId === tunnelId).length;
  };

  const calculateTunnelForwardPercentage = (tunnel: UserTunnel): number => {
    const totalUsed = getTunnelUsedForwards(tunnel.tunnelId);
    const totalLimit = tunnel.num || 0;
    // 无限制时返回0%
    if (tunnel.num === 99999) return 0;
    return totalLimit > 0 ? Math.min((totalUsed / totalLimit) * 100, 100) : 0;
  };

  const formatResetTime = (resetDay?: number): string => {
    if (resetDay === undefined || resetDay === null) return '';
    if (resetDay === 0) return '不重置';

    const now = new Date();
    const currentDay = now.getDate();

    let daysUntilReset;
    if (resetDay > currentDay) {
      daysUntilReset = resetDay - currentDay;
    } else if (resetDay < currentDay) {
      const nextMonth = new Date(now.getFullYear(), now.getMonth() + 1, resetDay);
      const diffTime = nextMonth.getTime() - now.getTime();
      daysUntilReset = Math.ceil(diffTime / (1000 * 60 * 60 * 24));
    } else {
      daysUntilReset = 0;
    }

    if (daysUntilReset === 0) {
      return '今日重置';
    } else if (daysUntilReset === 1) {
      return '明日重置';
    } else {
      return `${daysUntilReset}天后重置`;
    }
  };

  const groupedForwards = () => {
    const groups: { [key: string]: { tunnelName: string; forwards: Forward[] } } = {};
    forwardList.forEach(forward => {
      const tunnelName = forward.tunnelName || '未知隧道';
      if (!groups[tunnelName]) {
        groups[tunnelName] = {
          tunnelName,
          forwards: []
        };
      }
      groups[tunnelName].forwards.push(forward);
    });
    return Object.values(groups);
  };

  const formatInAddress = (ipString: string, port: number): string => {
    if (!ipString) return '';

    const items = ipString.split(',').map(item => item.trim()).filter(item => item);
    if (items.length === 0) return '';

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
    if (!port) return '';

    if (items.length === 1) {
      const ip = items[0];
      if (ip.includes(':') && !ip.startsWith('[')) {
        return `[${ip}]:${port}`;
      } else {
        return `${ip}:${port}`;
      }
    }

    const firstIp = items[0];
    let formattedFirstIp;
    if (firstIp.includes(':') && !firstIp.startsWith('[')) {
      formattedFirstIp = `[${firstIp}]`;
    } else {
      formattedFirstIp = firstIp;
    }

    return `${formattedFirstIp}:${port} (+${items.length - 1}个)`;
  };

  const formatRemoteAddress = (remoteAddr: string): string => {
    if (!remoteAddr) return '';

    const addresses = remoteAddr.split(',').map(addr => addr.trim()).filter(addr => addr);

    if (addresses.length === 0) return '';

    if (addresses.length === 1) {
      return addresses[0];
    }

    return `${addresses[0]} (+${addresses.length - 1})`;
  };

  const hasMultipleIps = (ipString: string): boolean => {
    if (!ipString) return false;
    const ips = ipString.split(',').map(ip => ip.trim()).filter(ip => ip);
    return ips.length > 1;
  };

  const hasMultipleRemoteAddresses = (remoteAddr: string): boolean => {
    if (!remoteAddr) return false;
    const addresses = remoteAddr.split(',').map(addr => addr.trim()).filter(addr => addr);
    return addresses.length > 1;
  };

  const showAddressModal = (ipString: string, port: number, title: string) => {
    if (!ipString) return;

    const items = ipString.split(',').map(item => item.trim()).filter(item => item);

    if (items.length <= 1) {
      copyToClipboard(formatInAddress(ipString, port));
      return;
    }

    // 检查是否已经包含端口
    const hasPort = /:\d+$/.test(items[0]);

    let formattedList;
    if (hasPort) {
      // 已经包含完整的 IP:Port 组合，直接使用
      formattedList = items.map((item, index) => ({
        id: index,
        ip: item,
        address: item,
        copying: false
      }));
    } else {
      // 只包含IP，需要添加端口
      formattedList = items.map((ip, index) => {
        let formattedAddress;
        if (ip.includes(':') && !ip.startsWith('[')) {
          formattedAddress = `[${ip}]:${port}`;
        } else {
          formattedAddress = `${ip}:${port}`;
        }
        return {
          id: index,
          ip: ip,
          address: formattedAddress,
          copying: false
        };
      });
    }

    setAddressList(formattedList);
    setAddressModalTitle(`${title} (${items.length}个)`);
    setAddressModalOpen(true);
  };

  const showRemoteAddressModal = (remoteAddr: string, title: string) => {
    if (!remoteAddr) return;

    const addresses = remoteAddr.split(',').map(addr => addr.trim()).filter(addr => addr);

    if (addresses.length <= 1) {
      copyToClipboard(remoteAddr);
      return;
    }

    const formattedList = addresses.map((address, index) => {
      return {
        id: index,
        ip: address,
        address: address,
        copying: false
      };
    });

    setAddressList(formattedList);
    setAddressModalTitle(`${title} (${addresses.length}个)`);
    setAddressModalOpen(true);
  };

  const copyToClipboard = async (text: string) => {
    try {
      await navigator.clipboard.writeText(text);
      toast.success(`已复制`);
    } catch (error) {
      toast.error('复制失败');
    }
  };

  const copyAddress = async (addressItem: AddressItem) => {
    try {
      setAddressList(prev => prev.map(item =>
        item.id === addressItem.id ? { ...item, copying: true } : item
      ));
      await copyToClipboard(addressItem.address);
    } catch (error) {
      toast.error('复制失败');
    } finally {
      setAddressList(prev => prev.map(item =>
        item.id === addressItem.id ? { ...item, copying: false } : item
      ));
    }
  };

  const copyAllAddresses = async () => {
    if (addressList.length === 0) return;
    const allAddresses = addressList.map(item => item.address).join('\n');
    await copyToClipboard(allAddresses);
  };

  const calculateForwardBillingFlow = (forward: Forward): number => {
    if (!forward) return 0;

    const inFlow = forward.inFlow || 0;
    const outFlow = forward.outFlow || 0;

    // 后端已按计费类型处理流量，前端直接使用入站+出站总和
    return inFlow + outFlow;
  };

  if (loading) {
    return (
      <div>
        <PageHeader title="仪表盘" desc="流量 / 隧道 / 转发一览" />
        <Loading text="正在加载数据..." />
      </div>
    );
  }

  const flowPct = calculateUsagePercentage('flow');
  const fwdPct = calculateUsagePercentage('forwards');

  return (
    <div>
      <PageHeader
        title="仪表盘"
        desc="流量 / 隧道 / 转发一览"
        actions={
          <Button variant="outline" size="sm" loading={refreshing} onClick={handleRefresh}>
            {!refreshing && <RefreshCw size={14} />}
            刷新
          </Button>
        }
      />

      {/* 统计卡片 */}
      <div className="grid grid-cols-2 gap-4 xl:grid-cols-4">
        <Stat
          label="总流量"
          icon={<Database size={18} />}
          accent="rgba(59,130,246,.16)"
          value={<GradientNum>{formatFlow(userInfo.flow, 'gb')}</GradientNum>}
          sub={userInfo.expTime ? <span>有效期至 {userInfo.expTime}</span> : <span>永久有效</span>}
        />
        <Stat
          label="已用流量"
          icon={<Activity size={18} />}
          accent="rgba(52,211,153,.16)"
          value={<GradientNum>{formatFlow(calculateUserTotalUsedFlow())}</GradientNum>}
          sub={
            <div className="space-y-1.5">
              <Progress value={userInfo.flow === 99999 ? 100 : flowPct} tone={userInfo.flow === 99999 ? "blue" : "auto"} />
              <div className="flex items-center justify-between gap-2">
                <span>{userInfo.flow === 99999 ? '无限制' : `${flowPct.toFixed(1)}%`}</span>
                {(userInfo.flowResetTime !== undefined && userInfo.flowResetTime !== null) && (
                  <span className="inline-flex items-center gap-1">
                    <Clock3 size={12} />
                    {formatResetTime(userInfo.flowResetTime)}
                  </span>
                )}
              </div>
            </div>
          }
        />
        <Stat
          label="转发配额"
          icon={<Network size={18} />}
          accent="rgba(148,163,184,.16)"
          value={<GradientNum>{formatNumber(userInfo.num || 0)}</GradientNum>}
          sub={<span>可用转发总数</span>}
        />
        <Stat
          label="已用转发"
          icon={<Link2 size={18} />}
          accent="rgba(251,191,36,.16)"
          value={<GradientNum>{forwardList.length}</GradientNum>}
          sub={
            <div className="space-y-1.5">
              <Progress value={userInfo.num === 99999 ? 100 : fwdPct} tone={userInfo.num === 99999 ? "blue" : "auto"} />
              <span>{userInfo.num === 99999 ? '无限制' : `${fwdPct.toFixed(1)}%`}</span>
            </div>
          }
        />
      </div>

      {/* 24小时流量统计图表 */}
      <Card className="mt-4">
        <CardHead
          title="24小时流量统计"
          desc="按小时补齐缺失时段，无数据小时记为 0"
        />
        <div className="px-5 pb-5">
          {statisticsFlows.length === 0 ? (
            <Empty text="暂无流量统计数据" />
          ) : (
            <div className="h-64 w-full lg:h-80">
              <ResponsiveContainer width="100%" height="100%">
                <AreaChart data={processFlowChartData()} margin={{ top: 8, right: 8, bottom: 0, left: 0 }}>
                  <defs>
                    <linearGradient id="flowFill" x1="0" y1="0" x2="0" y2="1">
                      <stop offset="0%" stopColor="#3b82f6" stopOpacity={0.16} />
                      <stop offset="100%" stopColor="#3b82f6" stopOpacity={0.02} />
                    </linearGradient>
                  </defs>
                  <CartesianGrid strokeDasharray="3 3" stroke="rgba(255,255,255,.06)" />
                  <XAxis
                    dataKey="time"
                    tick={{ fontSize: 12, fill: '#64748b' }}
                    tickLine={false}
                    axisLine={{ stroke: 'rgba(255,255,255,.08)', strokeWidth: 1 }}
                  />
                  <YAxis
                    tick={{ fontSize: 12, fill: '#64748b' }}
                    tickLine={false}
                    axisLine={{ stroke: 'rgba(255,255,255,.08)', strokeWidth: 1 }}
                    tickFormatter={(value) => {
                      if (!value || value <= 0) return '0';
                      if (value < 1024) return `${value}B`;
                      const k = 1024;
                      const sizes = ['B', 'K', 'M', 'G', 'T', 'P', 'E'];
                      const i = Math.min(Math.floor(Math.log(value) / Math.log(k)), sizes.length - 1);
                      return `${(value / Math.pow(k, i)).toFixed(1)}${sizes[i]}`;
                    }}
                  />
                  <Tooltip
                    content={({ active, payload, label }) => {
                      if (active && payload && payload.length) {
                        return (
                          <div className="rounded-xl border border-white/10 bg-[#151a23] px-3 py-2 shadow-xl">
                            <p className="font-medium text-slate-200">{`时间: ${label}`}</p>
                            <p className="text-blue-300">
                              {`流量: ${formatFlow((payload[0]?.value as number) || 0)}`}
                            </p>
                          </div>
                        );
                      }
                      return null;
                    }}
                  />
                  <Area
                    type="monotone"
                    dataKey="flow"
                    stroke="#3b82f6"
                    strokeWidth={2.5}
                    fill="url(#flowFill)"
                    dot={false}
                    activeDot={{ r: 4, stroke: '#3b82f6', strokeWidth: 2, fill: '#0d1117' }}
                  />
                </AreaChart>
              </ResponsiveContainer>
            </div>
          )}
        </div>
      </Card>

      {/* 隧道权限 - 管理员不显示 */}
      {!isAdmin && (
        <Card className="mt-4">
          <CardHead
            title="隧道权限"
            desc={userTunnels.length === 0 ? undefined : `共 ${userTunnels.length} 条隧道权限`}
            right={<Badge tone="slate">{userTunnels.length}</Badge>}
          />
          <div className="px-5 pb-5">
            {userTunnels.length === 0 ? (
              <Empty text="暂无隧道权限" />
            ) : (
              <div className="space-y-3">
                {userTunnels.map((tunnel) => {
                  const tunnelExpStatus = getExpStatus(tunnel.expTime);
                  const tunnelFlowPct = calculateTunnelFlowPercentage(tunnel);
                  const tunnelFwdPct = calculateTunnelForwardPercentage(tunnel);
                  return (
                    <div key={tunnel.id} className="rounded-xl border border-white/[0.07] bg-white/[0.02] p-4">
                      <div className="mb-3 flex flex-col gap-2 lg:flex-row lg:items-center lg:justify-between">
                        <h3 className="font-semibold text-white">
                          {tunnel.tunnelName} <span className="font-mono text-xs text-slate-500">ID: {tunnel.id}</span>
                        </h3>
                        <div className="flex flex-wrap items-center gap-2">
                          <Badge tone={tunnel.tunnelFlow === 1 ? "cyan" : "amber"}>
                            {tunnel.tunnelFlow === 1 ? '单向计费' : '双向计费'}
                          </Badge>
                          <Badge tone={tunnelExpStatus.tone} dot>{tunnelExpStatus.text}</Badge>
                          {(tunnel.flowResetTime !== undefined && tunnel.flowResetTime !== null) && (
                            <span className="inline-flex items-center gap-1 text-xs text-slate-500">
                              <CalendarClock size={13} />
                              {formatResetTime(tunnel.flowResetTime)}
                            </span>
                          )}
                        </div>
                      </div>

                      <div className="grid grid-cols-2 gap-4 lg:grid-cols-4">
                        <div>
                          <p className="mb-1 text-txs text-slate-500">流量配额</p>
                          <p className="font-semibold text-white">{formatFlow(tunnel.flow, 'gb')}</p>
                        </div>
                        <div>
                          <p className="mb-1 text-txs text-slate-500">已用流量</p>
                          <p className="font-semibold text-white">{formatFlow(calculateTunnelUsedFlow(tunnel))}</p>
                          <div className="mt-1.5">
                            <Progress value={tunnel.flow === 99999 ? 100 : tunnelFlowPct} tone={tunnel.flow === 99999 ? "blue" : "auto"} />
                          </div>
                        </div>
                        <div>
                          <p className="mb-1 text-txs text-slate-500">转发配额</p>
                          <p className="font-semibold text-white">{formatNumber(tunnel.num)}</p>
                        </div>
                        <div>
                          <p className="mb-1 text-txs text-slate-500">已用转发</p>
                          <p className="font-semibold text-white">{getTunnelUsedForwards(tunnel.tunnelId)}</p>
                          <div className="mt-1.5">
                            <Progress value={tunnel.num === 99999 ? 100 : tunnelFwdPct} tone={tunnel.num === 99999 ? "blue" : "auto"} />
                          </div>
                        </div>
                      </div>
                    </div>
                  );
                })}
              </div>
            )}
          </div>
        </Card>
      )}

      {/* 转发配置 */}
      <Card className="mt-4">
        <CardHead
          title="转发配置"
          desc={groupedForwards().length === 0 ? undefined : `按隧道分组，共 ${forwardList.length} 条转发`}
          right={<Badge tone="cyan">{forwardList.length}</Badge>}
        />
        <div className="px-5 pb-5">
          {groupedForwards().length === 0 ? (
            <Empty text="暂无转发配置" />
          ) : (
            <div className="space-y-4">
              {groupedForwards().map((group) => (
                <div key={group.tunnelName} className="rounded-xl border border-white/[0.07] bg-white/[0.02] p-4">
                  <div className="mb-3 flex items-center justify-between gap-2">
                    <h3 className="inline-flex items-center gap-2 font-semibold text-white">
                      <Server size={15} className="text-blue-300" />
                      {group.tunnelName}
                    </h3>
                    <Badge tone="cyan">{group.forwards.length} 个转发</Badge>
                  </div>

                  <div className="grid grid-cols-1 gap-3 sm:grid-cols-2 lg:grid-cols-3 2xl:grid-cols-4">
                    {group.forwards.map((forward) => (
                      <div key={forward.id} className="rounded-xl border border-white/[0.07] bg-white/[0.02] p-3">
                        <div className="space-y-2">
                          <h4 className="truncate text-sm font-medium text-white">{forward.name}</h4>
                          <div className="space-y-1.5">
                            <code
                              className={`block truncate rounded-lg border border-emerald-400/20 bg-emerald-400/10 px-2 py-1 font-mono text-xs text-emerald-300 ${hasMultipleIps(forward.inIp) ? 'cursor-pointer hover:bg-emerald-400/20' : ''}`}
                              onClick={() => hasMultipleIps(forward.inIp) && showAddressModal(forward.inIp, forward.inPort, '入口地址')}
                              title={formatInAddress(forward.inIp, forward.inPort)}
                            >
                              {formatInAddress(forward.inIp, forward.inPort)}
                            </code>
                            <div className="flex justify-center text-slate-500">
                              <ArrowDown size={13} />
                            </div>
                            <code
                              className={`block truncate rounded-lg border border-blue-400/20 bg-blue-400/10 px-2 py-1 font-mono text-xs text-blue-300 ${hasMultipleRemoteAddresses(forward.remoteAddr) ? 'cursor-pointer hover:bg-blue-400/20' : ''}`}
                              onClick={() => hasMultipleRemoteAddresses(forward.remoteAddr) && showRemoteAddressModal(forward.remoteAddr, '出口地址')}
                              title={formatRemoteAddress(forward.remoteAddr)}
                            >
                              {formatRemoteAddress(forward.remoteAddr)}
                            </code>
                          </div>

                          <div className="grid grid-cols-3 gap-1 border-t border-white/[0.06] pt-2 text-center text-xs">
                            <div>
                              <div className="mb-0.5 text-slate-500">上传</div>
                              <div className="truncate font-medium text-emerald-300">{formatFlow(forward.inFlow || 0)}</div>
                            </div>
                            <div>
                              <div className="mb-0.5 text-slate-500">下载</div>
                              <div className="truncate font-medium text-amber-300">{formatFlow(forward.outFlow || 0)}</div>
                            </div>
                            <div>
                              <div className="mb-0.5 text-slate-500">计费</div>
                              <div className="truncate font-medium text-blue-300">{formatFlow(calculateForwardBillingFlow(forward))}</div>
                            </div>
                          </div>
                        </div>
                      </div>
                    ))}
                  </div>
                </div>
              ))}
            </div>
          )}
        </div>
      </Card>

      {/* 地址列表弹窗 */}
      <Modal
        open={addressModalOpen}
        onClose={() => setAddressModalOpen(false)}
        title={addressModalTitle}
        width="max-w-2xl"
        footer={
          <Button variant="primary" size="sm" onClick={copyAllAddresses}>
            <Copy size={14} />
            复制全部
          </Button>
        }
      >
        <div className="max-h-60 space-y-2 overflow-y-auto">
          {addressList.map((item) => (
            <div key={item.id} className="flex items-center justify-between gap-3 rounded-lg border border-white/[0.07] bg-white/[0.02] p-3">
              <code className="mr-3 flex-1 break-all font-mono text-sm text-slate-200">{item.address}</code>
              <Button
                size="sm"
                variant="ghost"
                loading={item.copying}
                onClick={() => copyAddress(item)}
              >
                {!item.copying && <Check size={13} />}
                复制
              </Button>
            </div>
          ))}
        </div>
      </Modal>
    </div>
  );
}
