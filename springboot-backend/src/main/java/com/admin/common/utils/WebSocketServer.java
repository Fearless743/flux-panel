package com.admin.common.utils;


import com.admin.common.dto.GostConfigDto;
import com.admin.common.dto.GostDto;
import com.admin.common.task.CheckGostConfigAsync;
import com.admin.entity.Node;
import com.admin.service.NodeService;
import com.alibaba.fastjson.JSONObject;
import lombok.SneakyThrows;
import lombok.extern.slf4j.Slf4j;
import org.apache.commons.lang3.StringUtils;
import org.springframework.web.socket.CloseStatus;
import org.springframework.web.socket.TextMessage;
import org.springframework.web.socket.WebSocketSession;
import org.springframework.web.socket.handler.TextWebSocketHandler;

import javax.annotation.Resource;
import java.util.Objects;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.CopyOnWriteArraySet;
import java.util.concurrent.ConcurrentHashMap;
import java.util.concurrent.TimeUnit;
import java.util.UUID;


@Slf4j
public class WebSocketServer extends TextWebSocketHandler {

    @Resource
    NodeService nodeService;

    // 存储所有活跃的 WebSocket 连接（
    private static final CopyOnWriteArraySet<WebSocketSession> activeSessions = new CopyOnWriteArraySet<>();
    
    // 存储节点ID和对应的WebSocket session映射
    private static final ConcurrentHashMap<Long, WebSocketSession> nodeSessions = new ConcurrentHashMap<>();
    
    // 为每个session提供锁对象，防止并发发送消息
    private static final ConcurrentHashMap<String, Object> sessionLocks = new ConcurrentHashMap<>();
    
    // 存储等待响应的请求，key为requestId，value为CompletableFuture
    private static final ConcurrentHashMap<String, CompletableFuture<GostDto>> pendingRequests = new ConcurrentHashMap<>();
    
    // 存储请求ID与节点ID的映射关系，用于清理特定节点的请求
    private static final ConcurrentHashMap<String, Long> requestNodeMapping = new ConcurrentHashMap<>();

    //接受客户端消息
    @Override
    public void handleTextMessage(WebSocketSession session, TextMessage message) {
        try {
            if (StringUtils.isNoneBlank(message.getPayload())) {
                
                String id = session.getAttributes().get("id").toString();
                String type = session.getAttributes().get("type").toString();

                if (message.getPayload().contains("memory_usage")){
                    // 先发送确认消息
                    sendToUser(session, "{\"type\":\"call\"}");
                }else if (message.getPayload().contains("requestId")) {
                    log.info("收到消息: {}", message.getPayload());
                    // 处理命令响应消息
                    try {
                        JSONObject responseJson = JSONObject.parseObject(message.getPayload());
                        String requestId = responseJson.getString("requestId");
                        String responseMessage = responseJson.getString("message");
                        String responseType = responseJson.getString("type");
                        JSONObject responseData = responseJson.getJSONObject("data");
                        
                        if (requestId != null) {
                            CompletableFuture<GostDto> future = pendingRequests.remove(requestId);
                            // 同时清理请求-节点映射关系
                            requestNodeMapping.remove(requestId);
                            
                            if (future != null) {
                                GostDto result = new GostDto();
                                
                                // 根据响应类型处理不同的数据
                                if ("PingResponse".equals(responseType) && responseData != null) {
                                    // 特殊处理ping响应，将完整的响应数据返回
                                    result.setMsg(responseMessage != null ? responseMessage : "OK");
                                    result.setData(responseData); // 保存ping详细结果
                                } else {
                                    // 其他类型的响应
                                    result.setMsg(responseMessage != null ? responseMessage : "无响应消息");
                                    if (responseData != null) {
                                        result.setData(responseData);
                                    }
                                }
                                
                                future.complete(result);
                            }
                        }
                    } catch (Exception e) {
                        log.error("处理响应消息失败: {}", e.getMessage(), e);
                    }
                } else {
                    log.info("收到消息: {}", message.getPayload());
                }

                // 如果是节点类型，转发消息给其他会话
                if (Objects.equals(type, "1")) {
                    JSONObject jsonObject = new JSONObject();
                    jsonObject.put("id", id);
                    jsonObject.put("type", "info");
                    jsonObject.put("data", message.getPayload());
                    String broadcastMessage = jsonObject.toJSONString();
                    
                    // 异步处理广播消息，避免阻塞当前线程
                    for (WebSocketSession targetSession : activeSessions) {
                        if (targetSession != null && targetSession.isOpen() && !targetSession.equals(session)) {
                            sendToUser(targetSession, broadcastMessage);
                        }
                    }
                }
            }
        } catch (Exception e) {
            log.error("处理WebSocket消息时发生异常: {}", e.getMessage(), e);
        }
    }

    // 建立连接
    @Override
    public void afterConnectionEstablished(WebSocketSession session) {
        try {
            String id = session.getAttributes().get("id").toString();
            String type = session.getAttributes().get("type").toString();
            
            if (!Objects.equals(type, "1")) {
                // 网页管理员连接
                activeSessions.add(session);
                log.info("管理员连接建立，sessionId: {}", session.getId());
            } else {
                // 客户端节点连接
                Long nodeId = Long.valueOf(id);
                String version = (String) session.getAttributes().get("nodeVersion");
                
                log.info("节点 {} 连接建立，开始更新状态", nodeId);
                
                // 先添加到会话映射
                nodeSessions.put(nodeId, session);
                
                // 更新节点状态为在线
                Node node = nodeService.getById(nodeId);
                if (node != null) {
                    // 更新状态和版本信息
                    node.setStatus(1);
                    if (version != null) {
                        node.setVersion(version);
                    }
                    boolean updateResult = nodeService.updateById(node);
                    
                    if (updateResult) {
                        log.info("节点 {} 状态更新为在线成功，版本: {}", nodeId, version);
                        
                        // 广播节点上线状态给所有管理员
                        JSONObject res = new JSONObject();
                        res.put("id", id);
                        res.put("type", "status");
                        res.put("data", 1);
                        broadcastMessage(res.toJSONString());
                    } else {
                        log.error("节点 {} 状态更新失败", nodeId);
                    }
                } else {
                    log.error("节点 {} 不存在，无法更新状态", nodeId);
                    // 移除无效的会话
                    nodeSessions.remove(nodeId);
                }
            }

        } catch (Exception e) {
            log.error("建立连接时发生异常: {}", e.getMessage(), e);
            // 异常情况下，确保清理会话
            try {
                String id = session.getAttributes().get("id").toString();
                String type = session.getAttributes().get("type").toString();
                if (Objects.equals(type, "1")) {
                    Long nodeId = Long.valueOf(id);
                    nodeSessions.remove(nodeId);
                    log.warn("由于异常，移除节点 {} 的会话", nodeId);
                }
            } catch (Exception cleanupException) {
                log.error("清理异常会话时出错: {}", cleanupException.getMessage());
            }
        }
    }

    // 连接关闭后
    @Override
    public void afterConnectionClosed(WebSocketSession session, CloseStatus status) {
        try {
            String id = session.getAttributes().get("id").toString();
            String type = session.getAttributes().get("type").toString();
            String sessionId = session.getId();
            
            log.info("连接关闭，ID: {}, 类型: {}, 状态: {}", id, type, status);
            
            if (!Objects.equals(type, "1")) {
                // 管理员连接关闭
                boolean removed = activeSessions.remove(session);
                log.info("管理员连接关闭，sessionId: {}, 移除结果: {}", sessionId, removed);
            } else {
                // 客户端节点连接关闭
                Long nodeId = Long.valueOf(id);
                WebSocketSession removedSession = nodeSessions.remove(nodeId);
                
                log.info("节点 {} 连接关闭，开始更新状态为离线", nodeId);
                
                // 更新节点状态为离线
                Node node = nodeService.getById(nodeId);
                if (node != null) {
                    node.setStatus(0);
                    boolean updateResult = nodeService.updateById(node);
                    
                    if (updateResult) {
                        log.info("节点 {} 状态更新为离线成功", nodeId);
                        
                        JSONObject res = new JSONObject();
                        res.put("id", id);
                        res.put("type", "status");
                        res.put("data", 0);
                        broadcastMessage(res.toJSONString());
                    } else {
                        log.error("节点 {} 状态更新为离线失败", nodeId);
                    }
                } else {
                    log.warn("节点 {} 不存在，无法更新离线状态", nodeId);
                }
                
                // 清理该节点的待处理请求
                clearPendingRequestsForNode(nodeId);
            }
            
            // 清理session锁对象
            sessionLocks.remove(sessionId);

        } catch (Exception e) {
            log.error("关闭连接时发生异常: {}", e.getMessage(), e);
        }
    }

    // 点对点发送消息
    @SneakyThrows
    public static void sendToUser(WebSocketSession socketSession, String message) {
        if (socketSession != null && socketSession.isOpen()) {
            String sessionId = socketSession.getId();
            Object lock = sessionLocks.computeIfAbsent(sessionId, k -> new Object());
            
            synchronized (lock) {
                try {
                    if (socketSession.isOpen()) {
                        socketSession.sendMessage(new TextMessage(message));
                    }
                } catch (Exception e) {
                    log.error("发送WebSocket消息失败 [sessionId={}]: {}", sessionId, e.getMessage());
                    cleanupSession(socketSession);
                }
            }
        } else {
            cleanupSession(socketSession);
        }
    }
    
    /**
     * 清理失效的session，自动识别是节点session还是管理员session
     */
    private static void cleanupSession(WebSocketSession session) {
        if (session == null) return;
        
        String sessionId = session.getId();
        
        // 清理session锁
        sessionLocks.remove(sessionId);
        
        boolean removedFromAdmin = activeSessions.remove(session);
        
        if (!removedFromAdmin) {
            nodeSessions.entrySet().removeIf(entry -> {
                if (entry.getValue() == session) {
                    return true;
                }
                return false;
            });
        }
    }
    
    /**
     * 清理无效的session锁（定期清理任务）
     */
    public static void cleanupInvalidSessionLocks() {
        java.util.List<String> invalidSessionIds = new java.util.ArrayList<>();
        
        sessionLocks.keySet().forEach(sessionId -> {
            boolean isValidSession = false;
            
            // 检查是否为有效的管理员session
            for (WebSocketSession adminSession : activeSessions) {
                if (adminSession != null && sessionId.equals(adminSession.getId())) {
                    isValidSession = true;
                    break;
                }
            }
            
            // 检查是否为有效的节点session
            if (!isValidSession) {
                for (WebSocketSession nodeSession : nodeSessions.values()) {
                    if (nodeSession != null && sessionId.equals(nodeSession.getId())) {
                        isValidSession = true;
                        break;
                    }
                }
            }
            
            if (!isValidSession) {
                invalidSessionIds.add(sessionId);
            }
        });
        
        int cleanedCount = 0;
        for (String sessionId : invalidSessionIds) {
            if (sessionLocks.remove(sessionId) != null) {
                cleanedCount++;
            }
        }
        
        if (cleanedCount > 0) {
            log.info("清理了 {} 个无效的session锁", cleanedCount);
        }
    }

    // 广播消息
    public static void broadcastMessage(String message) {
        for (WebSocketSession session : activeSessions) {
            sendToUser(session, message);
        }
    }
    
    /**
     * 清理指定节点的待处理请求
     * 只清理属于该节点的未完成请求，避免影响其他节点的请求
     */
    private static void clearPendingRequestsForNode(Long nodeId) {
        if (nodeId == null) {
            log.warn("节点ID为空，无法清理待处理请求");
            return;
        }
        
        java.util.concurrent.atomic.AtomicInteger clearedCount = new java.util.concurrent.atomic.AtomicInteger(0);
        java.util.List<String> requestIdsToRemove = new java.util.ArrayList<>();
        
        // 找出属于该节点的请求
        requestNodeMapping.entrySet().forEach(entry -> {
            String requestId = entry.getKey();
            Long mappedNodeId = entry.getValue();
            
            if (nodeId.equals(mappedNodeId)) {
                CompletableFuture<GostDto> future = pendingRequests.get(requestId);
                if (future != null && !future.isDone()) {
                    // 完成该请求并设置错误信息
                    GostDto errorResult = new GostDto();
                    errorResult.setMsg("节点连接已断开");
                    future.complete(errorResult);
                    clearedCount.incrementAndGet();
                }
                requestIdsToRemove.add(requestId);
            }
        });
        
        // 批量清理映射关系和请求
        requestIdsToRemove.forEach(requestId -> {
            pendingRequests.remove(requestId);
            requestNodeMapping.remove(requestId);
        });
        
        if (clearedCount.get() > 0) {
            log.info("清理了节点 {} 的 {} 个待处理请求", nodeId, clearedCount.get());
        } else {
            log.debug("节点 {} 没有待处理的请求需要清理", nodeId);
        }
    }


    /**
     * 检查节点的实际连接状态，如果状态不一致则修复
     */
    public static boolean checkAndFixNodeStatus(NodeService nodeService, Long nodeId) {
        try {
            WebSocketSession session = nodeSessions.get(nodeId);
            boolean isConnected = session != null && session.isOpen();
            
            Node node = nodeService.getById(nodeId);
            if (node != null) {
                int currentStatus = node.getStatus();
                int expectedStatus = isConnected ? 1 : 0;
                
                if (currentStatus != expectedStatus) {
                    log.warn("节点 {} 状态不一致，数据库状态: {}, 实际连接状态: {}, 正在修复...", 
                            nodeId, currentStatus, expectedStatus);
                    
                    node.setStatus(expectedStatus);
                    boolean updateResult = nodeService.updateById(node);
                    
                    if (updateResult) {
                        log.info("节点 {} 状态修复成功，更新为: {}", nodeId, expectedStatus);
                        
                        // 广播状态变更
                        JSONObject res = new JSONObject();
                        res.put("id", nodeId.toString());
                        res.put("type", "status");
                        res.put("data", expectedStatus);
                        broadcastMessage(res.toJSONString());
                        
                        return true;
                    } else {
                        log.error("节点 {} 状态修复失败", nodeId);
                    }
                }
            }
            
            return isConnected;
        } catch (Exception e) {
            log.error("检查节点 {} 状态时发生异常: {}", nodeId, e.getMessage(), e);
            return false;
        }
    }
    
    /**
     * 获取节点的实际连接状态
     */
    public static boolean isNodeConnected(Long nodeId) {
        WebSocketSession session = nodeSessions.get(nodeId);
        return session != null && session.isOpen();
    }
    
    /**
     * 获取所有在线节点的ID列表
     */
    public static java.util.Set<Long> getConnectedNodeIds() {
        return nodeSessions.entrySet().stream()
                .filter(entry -> entry.getValue() != null && entry.getValue().isOpen())
                .map(java.util.Map.Entry::getKey)
                .collect(java.util.stream.Collectors.toSet());
    }
    
    /**
     * 获取指定节点的待处理请求数量
     */
    public static int getPendingRequestCount(Long nodeId) {
        if (nodeId == null) {
            return 0;
        }
        return (int) requestNodeMapping.entrySet().stream()
                .filter(entry -> nodeId.equals(entry.getValue()))
                .map(java.util.Map.Entry::getKey)
                .filter(requestId -> {
                    CompletableFuture<GostDto> future = pendingRequests.get(requestId);
                    return future != null && !future.isDone();
                })
                .count();
    }
    
    /**
     * 获取所有待处理请求的总数
     */
    public static int getTotalPendingRequestCount() {
        return (int) pendingRequests.entrySet().stream()
                .filter(entry -> entry.getValue() != null && !entry.getValue().isDone())
                .count();
    }
    
    /**
     * 清理所有已完成但未被移除的请求（定期清理任务）
     */
    public static void cleanupCompletedRequests() {
        java.util.List<String> completedRequestIds = new java.util.ArrayList<>();
        
        pendingRequests.entrySet().forEach(entry -> {
            String requestId = entry.getKey();
            CompletableFuture<GostDto> future = entry.getValue();
            
            if (future != null && future.isDone()) {
                completedRequestIds.add(requestId);
            }
        });
        
        int cleanedCount = 0;
        for (String requestId : completedRequestIds) {
            if (pendingRequests.remove(requestId) != null) {
                requestNodeMapping.remove(requestId);
                cleanedCount++;
            }
        }
        
        if (cleanedCount > 0) {
            log.info("清理了 {} 个已完成的请求", cleanedCount);
        }
    }
    
    /**
     * 获取WebSocket连接统计信息
     */
    public static java.util.Map<String, Object> getConnectionStats() {
        java.util.Map<String, Object> stats = new java.util.HashMap<>();
        
        // 节点连接统计
        int totalNodes = nodeSessions.size();
        int onlineNodes = (int) nodeSessions.entrySet().stream()
                .filter(entry -> entry.getValue() != null && entry.getValue().isOpen())
                .count();
        
        // 管理员连接统计
        int adminConnections = activeSessions.size();
        
        // 请求统计
        int totalPendingRequests = getTotalPendingRequestCount();
        int totalMappings = requestNodeMapping.size();
        
        stats.put("totalNodes", totalNodes);
        stats.put("onlineNodes", onlineNodes);
        stats.put("offlineNodes", totalNodes - onlineNodes);
        stats.put("adminConnections", adminConnections);
        stats.put("totalPendingRequests", totalPendingRequests);
        stats.put("totalRequestMappings", totalMappings);
        
        return stats;
    }
    
    /**
     * 执行全面的内存清理操作
     * 建议定期调用以防止内存泄漏
     */
    public static void performFullCleanup() {
        log.info("开始执行WebSocket全面清理操作...");
        
        // 获取清理前的统计信息
        java.util.Map<String, Object> statsBefore = getConnectionStats();
        
        // 执行各种清理操作
        cleanupCompletedRequests();
        cleanupInvalidSessionLocks();
        
        // 清理失效的节点session
        java.util.List<Long> invalidNodeIds = new java.util.ArrayList<>();
        nodeSessions.entrySet().forEach(entry -> {
            WebSocketSession session = entry.getValue();
            if (session == null || !session.isOpen()) {
                invalidNodeIds.add(entry.getKey());
            }
        });
        
        int removedNodeSessions = 0;
        for (Long nodeId : invalidNodeIds) {
            if (nodeSessions.remove(nodeId) != null) {
                removedNodeSessions++;
            }
        }
        
        // 清理失效的管理员session
        int removedAdminSessions = 0;
        java.util.Iterator<WebSocketSession> adminIterator = activeSessions.iterator();
        while (adminIterator.hasNext()) {
            WebSocketSession session = adminIterator.next();
            if (session == null || !session.isOpen()) {
                adminIterator.remove();
                removedAdminSessions++;
            }
        }
        
        // 获取清理后的统计信息
        java.util.Map<String, Object> statsAfter = getConnectionStats();
        
        log.info("WebSocket清理完成 - 清理前: {}, 清理后: {}, 移除节点session: {}, 移除管理员session: {}", 
                statsBefore, statsAfter, removedNodeSessions, removedAdminSessions);
    }

    public static GostDto send_msg(Long node_id, Object msg, String type) {
        WebSocketSession nodeSession = nodeSessions.get(node_id);

        if (nodeSession == null) {
            log.warn("发送消息失败：节点 {} 不在线或会话不存在", node_id);
            GostDto result = new GostDto();
            result.setMsg("节点不在线");
            return result;
        }

        if (!nodeSession.isOpen()) {
            log.warn("发送消息失败：节点 {} 连接已断开，清理会话", node_id);
            nodeSessions.remove(node_id);
            sessionLocks.remove(nodeSession.getId());
            GostDto result = new GostDto();
            result.setMsg("节点连接已断开");
            return result;
        }

        // 生成唯一的请求ID
        String requestId = UUID.randomUUID().toString();
        
        // 创建CompletableFuture用于等待响应
        CompletableFuture<GostDto> future = new CompletableFuture<>();
        pendingRequests.put(requestId, future);
        
        // 建立请求ID与节点ID的映射关系
        requestNodeMapping.put(requestId, node_id);

        try {
            JSONObject data = new JSONObject();
            data.put("type", type);
            data.put("data", msg);
            data.put("requestId", requestId);
            sendToUser(nodeSession, data.toJSONString());
            GostDto result = future.get(10, TimeUnit.SECONDS);
            
            log.debug("成功发送消息到节点 {} 并收到响应: {}", node_id, result.getMsg());
            return result;
            
        } catch (Exception e) {
            // 清理请求和映射关系
            pendingRequests.remove(requestId);
            requestNodeMapping.remove(requestId);
            
            GostDto result = new GostDto();
            if (e instanceof java.util.concurrent.TimeoutException) {
                result.setMsg("等待响应超时");
                log.warn("节点 {} 响应超时，可能存在连接问题", node_id);
            } else {
                result.setMsg("发送消息失败: " + e.getMessage());
                log.error("发送消息到节点 {} 失败: {}", node_id, e.getMessage(), e);
            }
            return result;
        }
    }

    
}
