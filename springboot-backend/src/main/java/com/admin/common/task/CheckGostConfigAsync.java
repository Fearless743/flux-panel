package com.admin.common.task;

import com.admin.common.dto.ConfigItem;
import com.admin.common.dto.GostConfigDto;
import com.admin.common.dto.GostDto;
import com.admin.common.utils.GostUtil;
import com.admin.entity.Forward;
import com.admin.entity.Node;
import com.admin.entity.SpeedLimit;
import com.admin.service.ForwardService;
import com.admin.service.NodeService;
import com.admin.service.SpeedLimitService;
import com.baomidou.mybatisplus.core.conditions.query.QueryWrapper;
import lombok.extern.slf4j.Slf4j;
import org.springframework.context.annotation.Configuration;
import org.springframework.scheduling.annotation.Async;
import org.springframework.scheduling.annotation.EnableScheduling;
import org.springframework.scheduling.annotation.Scheduled;
import org.springframework.stereotype.Service;

import javax.annotation.Resource;
import java.util.List;
import java.util.Objects;

@Slf4j
@Service
public class CheckGostConfigAsync {

    @Resource
    private NodeService nodeService;

    @Resource
    private ForwardService forwardService;

    @Resource
    private SpeedLimitService speedLimitService;


    /**
     * 清理孤立的Gost配置项
     */
    @Async
    public void cleanNodeConfigs(String node_id, GostConfigDto gostConfig) {
        Node node = nodeService.getById(node_id);
        if (node != null) {
            cleanOrphanedServices(gostConfig, node);
            cleanOrphanedChains(gostConfig, node);
            cleanOrphanedLimiters(gostConfig, node);
        }
    }

    /**
     * 清理孤立的服务
     */
    private void cleanOrphanedServices(GostConfigDto gostConfig, Node node) {
        if (gostConfig.getServices() == null) {
            return;
        }

        
        for (ConfigItem service : gostConfig.getServices()) {
            safeExecute(() -> {
                if (Objects.equals(service.getName(), "web_api")) {
                    return; // 排除API服务
                }
                
                String[] serviceIds = parseServiceName(service.getName());
                if (serviceIds.length == 4) {
                    String forwardId = serviceIds[0];
                    String userId = serviceIds[1];
                    String userTunnelId = serviceIds[2];
                    String type = serviceIds[3];
                    
                    if (Objects.equals(type, "tcp")) { // 只处理TCP，避免重复处理
                        Forward forward = forwardService.getById(forwardId);
                        if (forward == null) {
                            log.info("删除孤立的服务: {} (节点: {})", service.getName(), node.getId());
                            GostDto gostDto = GostUtil.DeleteService(node.getId(), forwardId + "_" + userId + "_" + userTunnelId);
                            System.out.println(gostDto);
                        }
                    }
                    if (Objects.equals(type, "tls")) {
                        Forward forward = forwardService.getById(forwardId);
                        if (forward == null) {
                            log.info("删除孤立的服务: {} (节点: {})", service.getName(), node.getId());
                            GostUtil.DeleteRemoteService(node.getId(), forwardId+"_"+userId+"_"+userTunnelId);
                        }
                    }
                }
            }, "清理服务 " + service.getName());
        }
    }

    /**
     * 清理孤立的链
     */
    private void cleanOrphanedChains(GostConfigDto gostConfig, Node node) {
        if (gostConfig.getChains() == null) {
            return;
        }
        

        for (ConfigItem chain : gostConfig.getChains()) {
            safeExecute(() -> {
                String[] serviceIds = parseServiceName(chain.getName());
                if (serviceIds.length == 4) {
                    String forwardId = serviceIds[0];
                    String userId = serviceIds[1];
                    String userTunnelId = serviceIds[2];
                    String type = serviceIds[3];
                    
                    if (Objects.equals(type, "chains")) {
                        Forward forward = forwardService.getById(forwardId);
                        if (forward == null) {
                            log.info("删除孤立的链: {} (节点: {})", chain.getName(), node.getId());
                            GostUtil.DeleteChains(node.getId(), forwardId+"_"+userId+"_"+userTunnelId);
                        }
                    }
                }
            }, "清理链 " + chain.getName());
        }
    }

    /**
     * 清理孤立的限流器
     */
    private void cleanOrphanedLimiters(GostConfigDto gostConfig, Node node) {
        if (gostConfig.getLimiters() == null) {
            return;
        }
        

        for (ConfigItem limiter : gostConfig.getLimiters()) {
            safeExecute(() -> {
                SpeedLimit speedLimit = speedLimitService.getById(limiter.getName());
                if (speedLimit == null) {
                    log.info("删除孤立的限流器: {} (节点: {})", limiter.getName(), node.getId());
                    GostUtil.DeleteLimiters(node.getId(), Long.parseLong(limiter.getName()));
                }
            }, "清理限流器 " + limiter.getName());
        }
    }

    /**
     * 安全执行操作，捕获异常
     */
    private void safeExecute(Runnable operation, String operationDesc) {
        try {
            operation.run();
        } catch (Exception e) {
            log.info("执行操作失败: {}", operationDesc, e);
        }
    }

    /**
     * 解析服务名称
     */
    private String[] parseServiceName(String serviceName) {
        return serviceName.split("_");
    }
}
