import { useState, useEffect, useRef } from "react";
import { RefreshCw } from "lucide-react";
import toast from 'react-hot-toast';
import axios from 'axios';

import { Button, Card, Badge, PageHeader, Empty, Loading, Progress } from "@/components/ui";
import { getNodeList } from "@/api";

interface NodeStatus {
  id: number;
  name: string;
  serverIp: string;
  version?: string;
  status: number; // 1: 在线, 0: 离线
  connectionStatus: 'online' | 'offline';
  systemInfo?: {
    cpuUsage: number;
    memoryUsage: number;
    uploadTraffic: number;
    downloadTraffic: number;
    uploadSpeed: number;
    downloadSpeed: number;
    uptime: number;
  } | null;
}

export default function NodeStatusPage() {
  const [nodeList, setNodeList] = useState<NodeStatus[]>([]);
  const [loading, setLoading] = useState(false);

  const websocketRef = useRef<WebSocket | null>(null);
  const reconnectTimerRef = useRef<NodeJS.Timeout | null>(null);
  const reconnectAttemptsRef = useRef(0);
  const maxReconnectAttempts = 5;

  useEffect(() => {
    loadNodes();
    initWebSocket();

    return () => {
      closeWebSocket();
    };
  }, []);

  // 加载节点列表
  const loadNodes = async () => {
    setLoading(true);
    try {
      const res = await getNodeList();
      if (res.code === 0) {
        setNodeList(res.data.map((node: any) => ({
          ...node,
          connectionStatus: node.status === 1 ? 'online' : 'offline',
          systemInfo: null,
        })));
      } else {
        toast.error(res.msg || '加载节点列表失败');
      }
    } catch (error) {
      toast.error('网络错误，请重试');
    } finally {
      setLoading(false);
    }
  };

  // 初始化WebSocket连接
  const initWebSocket = () => {
    if (websocketRef.current &&
        (websocketRef.current.readyState === WebSocket.OPEN ||
         websocketRef.current.readyState === WebSocket.CONNECTING)) {
      return;
    }

    if (websocketRef.current) {
      closeWebSocket();
    }

    // 构建WebSocket URL，使用axios的baseURL
    const baseUrl = axios.defaults.baseURL || (import.meta.env.VITE_API_BASE ? `${import.meta.env.VITE_API_BASE}/api/v1/` : '/api/v1/');
    const wsUrl = baseUrl.replace(/^http/, 'ws').replace(/\/api\/v1\/$/, '') + `/system-info?type=0&secret=${localStorage.getItem('token')}`;

    try {
      websocketRef.current = new WebSocket(wsUrl);

      websocketRef.current.onopen = () => {
        reconnectAttemptsRef.current = 0;
      };

      websocketRef.current.onmessage = (event) => {
        try {
          const data = JSON.parse(event.data);
          handleWebSocketMessage(data);
        } catch (error) {
          // 解析失败时不输出错误信息
        }
      };

      websocketRef.current.onerror = () => {
        // WebSocket错误时不输出错误信息
      };

      websocketRef.current.onclose = () => {
        websocketRef.current = null;
        attemptReconnect();
      };
    } catch (error) {
      attemptReconnect();
    }
  };

  // 处理WebSocket消息
  const handleWebSocketMessage = (data: any) => {
    const { id, type, data: messageData } = data;

    if (type === 'status') {
      setNodeList(prev => prev.map(node => {
        if (node.id == id) {
          return {
            ...node,
            connectionStatus: messageData === 1 ? 'online' : 'offline',
            systemInfo: messageData === 0 ? null : node.systemInfo
          };
        }
        return node;
      }));
    } else if (type === 'info') {
      setNodeList(prev => prev.map(node => {
        if (node.id == id) {
          try {
            let systemInfo;
            if (typeof messageData === 'string') {
              systemInfo = JSON.parse(messageData);
            } else {
              systemInfo = messageData;
            }

            const currentUpload = parseInt(systemInfo.bytes_transmitted) || 0;
            const currentDownload = parseInt(systemInfo.bytes_received) || 0;
            const currentUptime = parseInt(systemInfo.uptime) || 0;

            let uploadSpeed = 0;
            let downloadSpeed = 0;

            if (node.systemInfo && node.systemInfo.uptime) {
              const timeDiff = currentUptime - node.systemInfo.uptime;

              if (timeDiff > 0 && timeDiff <= 10) {
                const lastUpload = node.systemInfo.uploadTraffic || 0;
                const lastDownload = node.systemInfo.downloadTraffic || 0;

                const uploadDiff = currentUpload - lastUpload;
                const downloadDiff = currentDownload - lastDownload;

                const uploadReset = currentUpload < lastUpload;
                const downloadReset = currentDownload < lastDownload;

                if (!uploadReset && uploadDiff >= 0) {
                  uploadSpeed = uploadDiff / timeDiff;
                }

                if (!downloadReset && downloadDiff >= 0) {
                  downloadSpeed = downloadDiff / timeDiff;
                }
              }
            }

            return {
              ...node,
              connectionStatus: 'online',
              systemInfo: {
                cpuUsage: parseFloat(systemInfo.cpu_usage) || 0,
                memoryUsage: parseFloat(systemInfo.memory_usage) || 0,
                uploadTraffic: currentUpload,
                downloadTraffic: currentDownload,
                uploadSpeed: uploadSpeed,
                downloadSpeed: downloadSpeed,
                uptime: currentUptime
              }
            };
          } catch (error) {
            return node;
          }
        }
        return node;
      }));
    }
  };

  // 尝试重新连接
  const attemptReconnect = () => {
    if (reconnectAttemptsRef.current < maxReconnectAttempts) {
      reconnectAttemptsRef.current++;

      reconnectTimerRef.current = setTimeout(() => {
        initWebSocket();
      }, 3000 * reconnectAttemptsRef.current);
    }
  };

  // 关闭WebSocket连接
  const closeWebSocket = () => {
    if (reconnectTimerRef.current) {
      clearTimeout(reconnectTimerRef.current);
      reconnectTimerRef.current = null;
    }

    reconnectAttemptsRef.current = 0;

    if (websocketRef.current) {
      websocketRef.current.onopen = null;
      websocketRef.current.onmessage = null;
      websocketRef.current.onerror = null;
      websocketRef.current.onclose = null;

      if (websocketRef.current.readyState === WebSocket.OPEN ||
          websocketRef.current.readyState === WebSocket.CONNECTING) {
        websocketRef.current.close();
      }

      websocketRef.current = null;
    }

    setNodeList(prev => prev.map(node => ({
      ...node,
      connectionStatus: 'offline',
      systemInfo: null
    })));
  };

  // 格式化速度
  const formatSpeed = (bytesPerSecond: number): string => {
    if (!bytesPerSecond || bytesPerSecond <= 0) return '0 B/s';

    const k = 1024;
    const sizes = ['B/s', 'KB/s', 'MB/s', 'GB/s', 'TB/s', 'PB/s', 'EB/s'];
    const i = Math.min(Math.floor(Math.log(bytesPerSecond) / Math.log(k)), sizes.length - 1);

    return parseFloat((bytesPerSecond / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  };

  // 格式化开机时间
  const formatUptime = (seconds: number): string => {
    if (seconds === 0) return '-';

    const days = Math.floor(seconds / 86400);
    const hours = Math.floor((seconds % 86400) / 3600);
    const minutes = Math.floor((seconds % 3600) / 60);

    if (days > 0) {
      return `${days}天${hours}小时`;
    } else if (hours > 0) {
      return `${hours}小时${minutes}分钟`;
    } else {
      return `${minutes}分钟`;
    }
  };

  // 格式化流量
  const formatTraffic = (bytes: number): string => {
    if (!bytes || bytes <= 0) return '0 B';

    const k = 1024;
    const sizes = ['B', 'KB', 'MB', 'GB', 'TB', 'PB', 'EB'];
    const i = Math.min(Math.floor(Math.log(bytes) / Math.log(k)), sizes.length - 1);

    return parseFloat((bytes / Math.pow(k, i)).toFixed(2)) + ' ' + sizes[i];
  };

  // 获取使用率文字颜色（阈值逻辑保留：<=50 正常 / <=80 偏高 / >80 危险）
  const getUsageTextColor = (value: number, offline = false): string => {
    if (offline) return "text-slate-500";
    if (value <= 50) return "text-emerald-300";
    if (value <= 80) return "text-amber-300";
    return "text-red-300";
  };

  const isOnline = (node: NodeStatus) => node.connectionStatus === 'online';
  const onlineCount = nodeList.filter((n) => n.connectionStatus === 'online').length;

  return (
    <div>
      <PageHeader
        title="节点状态"
        desc={
          nodeList.length === 0
            ? "实时监控各节点运行状态"
            : `共 ${nodeList.length} 个节点 · 在线 ${onlineCount} · 离线 ${nodeList.length - onlineCount}`
        }
        actions={
          <Button variant="outline" size="sm" loading={loading} onClick={loadNodes}>
            {!loading && <RefreshCw size={14} />}
            刷新
          </Button>
        }
      />

      {loading ? (
        <Card>
          <Loading text="正在加载..." />
        </Card>
      ) : nodeList.length === 0 ? (
        <Card>
          <Empty text="暂无节点，请先到节点管理页面创建节点" />
        </Card>
      ) : (
        <Card className="!p-0 overflow-hidden">
          <div className="overflow-x-auto">
            <table className="flux-table min-w-[960px]" aria-label="节点状态列表">
              <thead>
                <tr>
                  <th>节点</th>
                  <th>状态</th>
                  <th className="min-w-[8rem]">CPU</th>
                  <th className="min-w-[8rem]">内存</th>
                  <th>网速</th>
                  <th>累计流量</th>
                  <th>运行时长</th>
                </tr>
              </thead>
              <tbody>
                {nodeList.map((node) => {
                  const online = isOnline(node);
                  const info = online ? node.systemInfo : null;
                  const cpu = info ? info.cpuUsage : 0;
                  const mem = info ? info.memoryUsage : 0;
                  return (
                    <tr key={node.id}>
                      <td>
                        <div className="min-w-0">
                          <div className="truncate text-sm font-medium text-white">{node.name}</div>
                          <div className="truncate font-mono text-xs text-slate-500" title={node.serverIp?.trim() || ''}>
                            {node.serverIp?.trim() || '-'}
                          </div>
                          <div className="mt-0.5 text-[11px] text-slate-600">
                            版本 {node.version || '未知'}
                          </div>
                        </div>
                      </td>
                      <td>
                        <Badge tone={online ? "green" : "red"} dot>
                          {online ? '在线' : '离线'}
                        </Badge>
                      </td>
                      <td>
                        <div className="min-w-[7rem] max-w-[10rem]">
                          <div className="mb-1 flex justify-between text-xs">
                            <span className="text-slate-500">CPU</span>
                            <span className={`font-mono ${getUsageTextColor(cpu, !online)}`}>
                              {info ? `${info.cpuUsage.toFixed(1)}%` : '-'}
                            </span>
                          </div>
                          <Progress value={info ? info.cpuUsage : 0} />
                        </div>
                      </td>
                      <td>
                        <div className="min-w-[7rem] max-w-[10rem]">
                          <div className="mb-1 flex justify-between text-xs">
                            <span className="text-slate-500">内存</span>
                            <span className={`font-mono ${getUsageTextColor(mem, !online)}`}>
                              {info ? `${info.memoryUsage.toFixed(1)}%` : '-'}
                            </span>
                          </div>
                          <Progress value={info ? info.memoryUsage : 0} />
                        </div>
                      </td>
                      <td>
                        <div className="space-y-0.5 font-mono text-xs">
                          <div className="text-blue-300">
                            ↑ {info ? formatSpeed(info.uploadSpeed) : '-'}
                          </div>
                          <div className="text-emerald-300">
                            ↓ {info ? formatSpeed(info.downloadSpeed) : '-'}
                          </div>
                        </div>
                      </td>
                      <td>
                        <div className="space-y-0.5 font-mono text-xs text-slate-400">
                          <div>
                            ↑ {info ? formatTraffic(info.uploadTraffic) : '-'}
                          </div>
                          <div>
                            ↓ {info ? formatTraffic(info.downloadTraffic) : '-'}
                          </div>
                        </div>
                      </td>
                      <td>
                        <span className="whitespace-nowrap text-xs text-slate-400">
                          {info ? formatUptime(info.uptime) : '-'}
                        </span>
                      </td>
                    </tr>
                  );
                })}
              </tbody>
            </table>
          </div>
        </Card>
      )}
    </div>
  );
}
