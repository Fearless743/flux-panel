import { useState, useEffect, useRef } from "react";
import { Button } from "@heroui/button";
import { Chip } from "@heroui/chip";
import { Spinner } from "@heroui/spinner";
import { Progress } from "@heroui/progress";
import toast from 'react-hot-toast';
import axios from 'axios';

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

  // 获取进度条颜色
  const getProgressColor = (value: number, offline = false): "default" | "primary" | "secondary" | "success" | "warning" | "danger" => {
    if (offline) return "default";
    if (value <= 50) return "success";
    if (value <= 80) return "warning";
    return "danger";
  };

  const cellClass = "px-3 py-3 align-middle";
  const isOnline = (node: NodeStatus) => node.connectionStatus === 'online';
  const onlineCount = nodeList.filter((n) => n.connectionStatus === 'online').length;

  return (
    <div className="px-3 lg:px-6 py-8">
      {/* 页面头部 */}
      <div className="flex flex-col sm:flex-row sm:items-center justify-between gap-3 mb-6">
        <div className="min-w-0">
          <h1 className="page-title">节点状态</h1>
          <p className="page-desc">
            共 {nodeList.length} 个节点
            {nodeList.length > 0 && (
              <span className="text-default-400"> · 在线 {onlineCount} · 离线 {nodeList.length - onlineCount}</span>
            )}
          </p>
        </div>

        <div className="flex items-center gap-2 flex-shrink-0">
          <Button
            size="sm"
            variant="flat"
            color="default"
            onPress={loadNodes}
            isLoading={loading}
          >
            刷新
          </Button>
        </div>
      </div>

      {/* 节点状态列表 */}
      {loading ? (
        <div className="flex items-center justify-center h-64">
          <div className="flex items-center gap-3">
            <Spinner size="sm" />
            <span className="text-default-500 text-sm">正在加载...</span>
          </div>
        </div>
      ) : nodeList.length === 0 ? (
        <div className="text-center py-16 panel-card border border-divider shadow-panel bg-content1">
          <div className="flex flex-col items-center gap-4">
            <div className="empty-state-icon">
              <svg className="w-8 h-8" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={1.5} d="M5 12h14M5 12l4-4m-4 4l4 4" />
              </svg>
            </div>
            <div>
              <h3 className="text-lg font-semibold text-foreground">暂无节点</h3>
              <p className="text-default-500 text-sm mt-1">还没有创建任何节点，请先到节点管理页面创建节点</p>
            </div>
          </div>
        </div>
      ) : (
        <div className="panel-table-wrap">
          <table className="w-full text-sm min-w-[960px]" aria-label="节点状态列表">
            <thead>
              <tr className="bg-default-50 text-default-600 font-medium text-xs">
                <th className="px-3 py-2.5 text-left">节点</th>
                <th className="px-3 py-2.5 text-left">状态</th>
                <th className="px-3 py-2.5 text-left min-w-[8rem]">CPU</th>
                <th className="px-3 py-2.5 text-left min-w-[8rem]">内存</th>
                <th className="px-3 py-2.5 text-left">网速</th>
                <th className="px-3 py-2.5 text-left">累计流量</th>
                <th className="px-3 py-2.5 text-left">运行时长</th>
              </tr>
            </thead>
            <tbody className="divide-y divide-divider">
              {nodeList.map((node) => {
                const online = isOnline(node);
                const info = online ? node.systemInfo : null;
                return (
                  <tr key={node.id} className="bg-content1 hover:bg-default-50/70 transition-colors">
                    <td className={cellClass}>
                      <div className="min-w-0">
                        <div className="font-medium text-foreground text-sm truncate">{node.name}</div>
                        <div className="text-xs text-default-500 font-mono truncate" title={node.serverIp?.trim() || ''}>
                          {node.serverIp?.trim() || '-'}
                        </div>
                        <div className="text-[11px] text-default-400 mt-0.5">
                          版本 {node.version || '未知'}
                        </div>
                      </div>
                    </td>
                    <td className={cellClass}>
                      <Chip
                        color={online ? 'success' : 'danger'}
                        variant="flat"
                        size="sm"
                        className="text-xs"
                        startContent={
                          <span className={`inline-block w-1.5 h-1.5 rounded-full ml-1 ${online ? 'bg-success' : 'bg-danger'}`} />
                        }
                      >
                        {online ? '在线' : '离线'}
                      </Chip>
                    </td>
                    <td className={cellClass}>
                      <div className="min-w-[7rem] max-w-[10rem]">
                        <div className="flex justify-between text-xs mb-1">
                          <span className="text-default-500">CPU</span>
                          <span className="font-mono text-foreground">
                            {info ? `${info.cpuUsage.toFixed(1)}%` : '-'}
                          </span>
                        </div>
                        <Progress
                          value={info ? info.cpuUsage : 0}
                          color={getProgressColor(info ? info.cpuUsage : 0, !online)}
                          size="sm"
                          aria-label={`${node.name} CPU使用率`}
                        />
                      </div>
                    </td>
                    <td className={cellClass}>
                      <div className="min-w-[7rem] max-w-[10rem]">
                        <div className="flex justify-between text-xs mb-1">
                          <span className="text-default-500">内存</span>
                          <span className="font-mono text-foreground">
                            {info ? `${info.memoryUsage.toFixed(1)}%` : '-'}
                          </span>
                        </div>
                        <Progress
                          value={info ? info.memoryUsage : 0}
                          color={getProgressColor(info ? info.memoryUsage : 0, !online)}
                          size="sm"
                          aria-label={`${node.name} 内存使用率`}
                        />
                      </div>
                    </td>
                    <td className={cellClass}>
                      <div className="text-xs space-y-0.5 font-mono">
                        <div className="text-primary">
                          ↑ {info ? formatSpeed(info.uploadSpeed) : '-'}
                        </div>
                        <div className="text-success">
                          ↓ {info ? formatSpeed(info.downloadSpeed) : '-'}
                        </div>
                      </div>
                    </td>
                    <td className={cellClass}>
                      <div className="text-xs space-y-0.5 font-mono">
                        <div className="text-default-600">
                          ↑ {info ? formatTraffic(info.uploadTraffic) : '-'}
                        </div>
                        <div className="text-default-600">
                          ↓ {info ? formatTraffic(info.downloadTraffic) : '-'}
                        </div>
                      </div>
                    </td>
                    <td className={cellClass}>
                      <span className="text-xs text-default-600 whitespace-nowrap">
                        {info ? formatUptime(info.uptime) : '-'}
                      </span>
                    </td>
                  </tr>
                );
              })}
            </tbody>
          </table>
        </div>
      )}
    </div>
  );
}
