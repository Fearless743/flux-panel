import { useState, useEffect } from 'react';
import { useNavigate } from 'react-router-dom';
import toast from 'react-hot-toast';
import { ArrowLeft, Plus, Server, Trash2, Check } from "lucide-react";
import { reinitializeBaseURL } from '@/api/network';
import {
  getPanelAddresses,
  savePanelAddress,
  setCurrentPanelAddress,
  deletePanelAddress,
  validatePanelAddress,
} from '@/utils/panel';
import { Button, Card, CardHead, Field, Input, Badge, Empty } from "@/components/ui";

interface PanelAddress {
  name: string;
  address: string;
  inx: boolean;
}


export const SettingsPage = () => {
  const navigate = useNavigate();
  const [panelAddresses, setPanelAddresses] = useState<PanelAddress[]>([]);
  const [newName, setNewName] = useState('');
  const [newAddress, setNewAddress] = useState('');


  const setPanelAddressesFunc = (newAddress: PanelAddress[]) => {
    setPanelAddresses(newAddress);
  }

  // 加载面板地址列表
  const loadPanelAddresses = async () => {
    (window as any).setPanelAddresses = setPanelAddressesFunc
    getPanelAddresses();
  };

  // 添加新面板地址
  const addPanelAddress = async () => {
    if (!newName.trim() || !newAddress.trim()) {
      toast.error('请输入名称和地址');
      return;
    }

    // 验证地址格式
    if (!validatePanelAddress(newAddress.trim())) {
      toast.error('地址格式不正确，请检查：\n• 必须是完整的URL格式\n• 必须以 http:// 或 https:// 开头\n• 支持域名、IPv4、IPv6 地址\n• 端口号范围：1-65535\n• 示例：http://192.168.1.100:3000');
      return;
    }
    (window as any).setPanelAddresses = setPanelAddressesFunc
    savePanelAddress(newName.trim(), newAddress.trim());
    setNewName('');
    setNewAddress('');
    toast.success('添加成功');
  };

  // 设置当前面板地址
  const setCurrentPanel = async (name: string) => {
    (window as any).setPanelAddresses = setPanelAddressesFunc
    setCurrentPanelAddress(name);
    reinitializeBaseURL();
  };

  // 删除面板地址
  const handleDeletePanelAddress = async (name: string) => {
    (window as any).setPanelAddresses = setPanelAddressesFunc
    deletePanelAddress(name);
    reinitializeBaseURL();
    toast.success('删除成功');
  };

  // 页面加载时获取数据
  useEffect(() => {
    loadPanelAddresses();
  }, []);

  const handleKeyPress = (e: React.KeyboardEvent) => {
    if (e.key === 'Enter') {
      addPanelAddress();
    }
  };

  return (
    <div className="relative min-h-screen">
      <div className="app-bg" />
      <div className="relative mx-auto w-full max-w-4xl px-4 py-6">
        <div className="mb-5 flex items-center gap-3">
          <Button variant="outline" size="icon" onClick={() => navigate(-1)} aria-label="返回">
            <ArrowLeft size={17} />
          </Button>
          <div>
            <h1 className="text-gradient text-t2xl font-bold tracking-tight">面板设置</h1>
            <p className="mt-0.5 text-tsm text-slate-400">管理已保存的面板地址，随时切换当前连接</p>
          </div>
        </div>

        <div className="space-y-5">
          {/* 添加新地址 */}
          <Card>
            <CardHead title="添加新面板地址" desc="保存后可在列表中一键切换" />
            <div className="space-y-4 px-5 pb-5">
              <div className="grid grid-cols-1 gap-4 sm:grid-cols-2">
                <Field label="名称">
                  <Input
                    placeholder="请输入面板名称"
                    value={newName}
                    onChange={(e) => setNewName(e.target.value)}
                    onKeyDown={handleKeyPress}
                  />
                </Field>
                <Field label="地址">
                  <Input
                    placeholder="http://192.168.1.100:3000"
                    value={newAddress}
                    onChange={(e) => setNewAddress(e.target.value)}
                    onKeyDown={handleKeyPress}
                  />
                </Field>
              </div>
              <Button variant="primary" onClick={addPanelAddress}>
                <Plus size={15} />
                添加
              </Button>
            </div>
          </Card>

          {/* 地址列表 */}
          <Card>
            <CardHead
              title="已保存的面板地址"
              desc={panelAddresses.length > 0 ? `共 ${panelAddresses.length} 个` : undefined}
            />
            <div className="px-5 pb-5">
              {panelAddresses.length === 0 ? (
                <Empty text="暂无保存的面板地址" />
              ) : (
                <div className="space-y-3">
                  {panelAddresses.map((panel, index) => (
                    <div
                      key={index}
                      className="flex items-center justify-between gap-3 rounded-xl border border-white/[0.07] bg-white/[0.03] p-4"
                    >
                      <div className="flex min-w-0 flex-1 items-start gap-3">
                        <div className="flex h-9 w-9 shrink-0 items-center justify-center rounded-lg border border-white/10 bg-white/[0.05] text-blue-300">
                          <Server size={16} />
                        </div>
                        <div className="min-w-0 flex-1">
                          <div className="flex flex-wrap items-center gap-2">
                            <span className="truncate text-tbase font-semibold text-white">{panel.name}</span>
                            {panel.inx && (
                              <Badge tone="green" dot>当前</Badge>
                            )}
                          </div>
                          <p className="mono mt-1 truncate text-txs text-slate-400">{panel.address}</p>
                        </div>
                      </div>
                      <div className="flex shrink-0 items-center gap-2">
                        {!panel.inx && (
                          <Button
                            size="sm"
                            variant="soft"
                            onClick={() => setCurrentPanel(panel.name)}
                          >
                            <Check size={14} />
                            设为当前
                          </Button>
                        )}
                        <Button
                          size="sm"
                          variant="danger"
                          onClick={() => handleDeletePanelAddress(panel.name)}
                        >
                          <Trash2 size={14} />
                          删除
                        </Button>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </div>
          </Card>
        </div>
      </div>
    </div>
  );
};
