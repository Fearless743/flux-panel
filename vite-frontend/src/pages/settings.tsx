import { useState, useEffect } from 'react';
import { Input } from "@heroui/input";
import { Button } from "@heroui/button";
import { Card, CardBody } from "@heroui/card";
import { useNavigate } from 'react-router-dom';
import toast from 'react-hot-toast';
import { reinitializeBaseURL } from '@/api/network';

interface PanelAddress {
  name: string;
  address: string;
}

export const SettingsPage = () => {
  const navigate = useNavigate();
  const [panelAddresses, setPanelAddresses] = useState<PanelAddress[]>([]);
  const [currentAddress, setCurrentAddress] = useState<string>('');
  const [newName, setNewName] = useState('');
  const [newAddress, setNewAddress] = useState('');

  // 验证面板地址格式
  const validatePanelAddress = (address: string): boolean => {
    try {
      // 基本格式检查：必须以 http:// 或 https:// 开头
      if (!address.startsWith('http://') && !address.startsWith('https://')) {
        return false;
      }

      // 使用URL构造函数验证完整URL格式
      const url = new URL(address);
      
      // 检查主机名不能为空
      if (!url.hostname || url.hostname.trim() === '') {
        return false;
      }
      
      // 检查主机名
      const hostname = url.hostname;
      
      // 支持 localhost
      if (hostname === 'localhost') {
        return true;
      }
      
      // 支持 IPv4 地址
      const ipv4Pattern = /^(\d{1,3}\.){3}\d{1,3}$/;
      if (ipv4Pattern.test(hostname)) {
        const parts = hostname.split('.');
        return parts.every(part => {
          const num = parseInt(part);
          return num >= 0 && num <= 255;
        });
      }
      
      // 支持 IPv6 地址
      const ipv6Pattern = /^\[([0-9a-fA-F]{1,4}:){7}[0-9a-fA-F]{1,4}\]$|^\[([0-9a-fA-F]{1,4}:)*:([0-9a-fA-F]{1,4}:)*[0-9a-fA-F]{1,4}\]$/;
      if (ipv6Pattern.test(hostname)) {
        return true;
      }
      
      // 支持域名
      const domainPattern = /^[a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?(\.[a-zA-Z0-9]([a-zA-Z0-9-]{0,61}[a-zA-Z0-9])?)*\.[a-zA-Z]{2,}$/;
      if (domainPattern.test(hostname)) {
        return true;
      }
      
      return false;
    } catch (error) {
      // URL构造函数失败说明格式不正确
      return false;
    }
  };

  // 加载面板地址列表
  const loadPanelAddresses = () => {
    try {
      if (typeof (window as any).AndroidInterface !== 'undefined') {
        const addressesStr = (window as any).AndroidInterface.getPanelAddresses();
        const currentAddr = (window as any).AndroidInterface.getCurrentPanelAddress();
        
        if (addressesStr && addressesStr.trim()) {
          const addresses = addressesStr.split(',').filter((item: string) => item.trim()).map((item: string) => {
            const [name, address] = item.split('|');
            return { name: name || '未命名', address: address || '' };
          });
          setPanelAddresses(addresses);
        } else {
          // 如果没有数据或数据为空，清空列表
          setPanelAddresses([]);
        }
        
        setCurrentAddress(currentAddr || '');
      }
    } catch (error) {
      console.error('加载面板地址失败:', error);
      setPanelAddresses([]);
    }
  };

  // 添加新面板地址
  const addPanelAddress = () => {
    if (!newName.trim() || !newAddress.trim()) {
      toast.error('请输入名称和地址');
      return;
    }

    // 验证地址格式
    if (!validatePanelAddress(newAddress.trim())) {
      toast.error('地址格式不正确，请检查：\n• 必须是完整的URL格式\n• 必须以 http:// 或 https:// 开头\n• 支持域名、IPv4、IPv6 地址\n• 端口号范围：1-65535\n• 示例：http://192.168.1.100:3000');
      return;
    }

    try {
      if (typeof (window as any).AndroidInterface !== 'undefined') {
        (window as any).AndroidInterface.savePanelAddress(newName.trim(), newAddress.trim());
        toast.success('添加成功');
        setNewName('');
        setNewAddress('');
        loadPanelAddresses();
      }
    } catch (error) {
      toast.error('添加失败' + error);
    }
  };

  // 设置当前面板地址
  const setCurrentPanel = (address: string) => {
    try {
      if (typeof (window as any).AndroidInterface !== 'undefined') {
        (window as any).AndroidInterface.setCurrentPanelAddress(address);
        setCurrentAddress(address);
        reinitializeBaseURL();
        toast.success('设置成功，API地址已更新');
      }
    } catch (error) {
      toast.error('设置失败');
    }
  };

  // 删除面板地址
  const deletePanelAddress = (name: string, address: string) => {
    try {
      if (typeof (window as any).AndroidInterface !== 'undefined') {
        (window as any).AndroidInterface.deletePanelAddress(name, address);
        
        // 如果删除的是当前选中的地址，重新初始化baseURL
        if (currentAddress === address) {
          setCurrentAddress('');
          reinitializeBaseURL();
          toast.success('删除成功，当前面板地址已清空');
        } else {
          toast.success('删除成功');
        }
        
        loadPanelAddresses();
      }
    } catch (error) {
      toast.error('删除失败');
    }
  };

  // 页面加载时获取数据
  useEffect(() => {
    loadPanelAddresses();
  }, []);

  return (
    <div className="min-h-screen bg-gray-50 dark:bg-gray-900">
      {/* 顶部导航 */}
      <div className="bg-white dark:bg-gray-800 border-b border-gray-200 dark:border-gray-700">
        <div className="max-w-4xl mx-auto px-4 py-4">
          <div className="flex items-center gap-3">
            <Button
              isIconOnly
              variant="light"
              onClick={() => navigate(-1)}
              className="text-gray-600 dark:text-gray-300"
            >
              <svg className="w-5 h-5" fill="none" stroke="currentColor" viewBox="0 0 24 24">
                <path strokeLinecap="round" strokeLinejoin="round" strokeWidth={2} d="M15 19l-7-7 7-7" />
              </svg>
            </Button>
            <h1 className="text-xl font-semibold text-gray-900 dark:text-white">面板设置</h1>
          </div>
        </div>
      </div>

      {/* 内容区域 */}
      <div className="max-w-4xl mx-auto px-4 py-6">
        <div className="space-y-6">
          {/* 添加新地址 */}
          <Card className="border border-gray-200 dark:border-gray-700">
            <CardBody className="p-6">
              <h2 className="text-lg font-medium text-gray-900 dark:text-white mb-4">添加新面板地址</h2>
              <div className="space-y-4">
                <div className="grid grid-cols-1 sm:grid-cols-2 gap-4">
                  <Input
                    label="名称"
                    placeholder="请输入面板名称"
                    value={newName}
                    onChange={(e) => setNewName(e.target.value)}
                  />
                  <Input
                    label="地址"
                    placeholder="http://192.168.1.100:3000"
                    value={newAddress}
                    onChange={(e) => setNewAddress(e.target.value)}
                  />
                </div>
                <Button color="primary" onClick={addPanelAddress}>
                  添加
                </Button>
              </div>
            </CardBody>
          </Card>

          {/* 地址列表 */}
          <Card className="border border-gray-200 dark:border-gray-700">
            <CardBody className="p-6">
              <h2 className="text-lg font-medium text-gray-900 dark:text-white mb-4">已保存的面板地址</h2>
              {panelAddresses.length === 0 ? (
                <p className="text-gray-500 dark:text-gray-400 text-center py-8">暂无保存的面板地址</p>
              ) : (
                <div className="space-y-3">
                  {panelAddresses.map((panel, index) => (
                    <div key={index} className="border border-gray-200 dark:border-gray-600 rounded-lg p-4">
                      <div className="flex items-center justify-between">
                        <div className="flex-1">
                          <div className="flex items-center gap-2">
                            <span className="font-medium text-gray-900 dark:text-white">{panel.name}</span>
                            {currentAddress === panel.address && (
                              <span className="px-2 py-1 bg-green-100 dark:bg-green-500/20 text-green-700 dark:text-green-300 text-xs rounded">
                                当前
                              </span>
                            )}
                          </div>
                          <p className="text-sm text-gray-500 dark:text-gray-400 mt-1">{panel.address}</p>
                        </div>
                        <div className="flex items-center gap-2">
                          {currentAddress !== panel.address && (
                            <Button
                              size="sm"
                              color="primary"
                              variant="flat"
                              onClick={() => setCurrentPanel(panel.address)}
                            >
                              设为当前
                            </Button>
                          )}
                          <Button
                            size="sm"
                            color="danger"
                            variant="light"
                            onClick={() => deletePanelAddress(panel.name, panel.address)}
                          >
                            删除
                          </Button>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>
              )}
            </CardBody>
          </Card>
        </div>
      </div>
    </div>
  );
};
