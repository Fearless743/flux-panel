package com.admin.common.task;

import com.admin.common.utils.WebSocketServer;
import lombok.extern.slf4j.Slf4j;
import org.springframework.scheduling.annotation.Scheduled;
import org.springframework.stereotype.Component;

/**
 * WebSocket资源清理定时任务
 * 定期清理已完成的请求、无效的session锁等，防止内存泄漏
 */
@Slf4j
@Component
public class WebSocketCleanupTask {

    /**
     * 每5分钟执行一次轻量级清理
     * 清理已完成的请求和无效的session锁
     */
    @Scheduled(fixedRate = 5 * 60 * 1000) // 5分钟
    public void lightweightCleanup() {
        try {
            log.debug("开始执行WebSocket轻量级清理...");
            
            // 清理已完成的请求
            WebSocketServer.cleanupCompletedRequests();
            
            // 清理无效的session锁
            WebSocketServer.cleanupInvalidSessionLocks();
            
        } catch (Exception e) {
            log.error("WebSocket轻量级清理失败", e);
        }
    }

    /**
     * 每30分钟执行一次全面清理
     * 包括所有类型的资源清理
     */
    @Scheduled(fixedRate = 30 * 60 * 1000) // 30分钟
    public void fullCleanup() {
        try {
            log.info("开始执行WebSocket全面清理...");
            
            // 执行全面清理
            WebSocketServer.performFullCleanup();
            
        } catch (Exception e) {
            log.error("WebSocket全面清理失败", e);
        }
    }
} 