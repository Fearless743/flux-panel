package com.admin.common.utils;

import com.admin.entity.*;
import com.alibaba.fastjson.JSONArray;
import com.alibaba.fastjson.JSONObject;
import org.apache.commons.lang3.StringUtils;

import java.util.List;
import java.util.Map;
import java.util.Objects;

public class GostUtil {


    public static void AddLimiters(Long node_id, Long name, String speed) {
        JSONObject data = createLimiterData(name, speed);
        WebSocketServer.send_msg(node_id, data, "AddLimiters");
    }

    public static void UpdateLimiters(Long node_id, Long name, String speed) {
        JSONObject data = createLimiterData(name, speed);
        JSONObject req = new JSONObject();
        req.put("limiter", name + "");
        req.put("data", data);
        WebSocketServer.send_msg(node_id, req, "UpdateLimiters");
    }

    public static void DeleteLimiters(Long node_id, Long name) {
        JSONObject req = new JSONObject();
        req.put("limiter", name + "");
        WebSocketServer.send_msg(node_id, req, "DeleteLimiters");
    }

    public static void AddChains(Long node_id, List<ChainTunnel> chainTunnels, Map<Long, Node> node_s) {
        JSONArray nodes = new JSONArray();
        for (ChainTunnel chainTunnel : chainTunnels) {
            JSONObject dialer = new JSONObject();
            dialer.put("type", chainTunnel.getProtocol());

            JSONObject connector = new JSONObject();
            connector.put("type", "relay");

            Node node_info = node_s.get(chainTunnel.getNodeId());
            JSONObject node = new JSONObject();
            node.put("name", "node_" + chainTunnel.getInx());
            node.put("addr", node_info.getServerIp() + ":" + chainTunnel.getPort());
            node.put("connector", connector);
            node.put("dialer", dialer);

            if (StringUtils.isNotBlank(node_info.getInterfaceName())) {
                node.put("interface", node_info.getInterfaceName());
            }

            nodes.add(node);
        }
        JSONObject hop = new JSONObject();
        hop.put("name", "hop_" + chainTunnels.getFirst().getTunnelId());

        JSONObject selector = new JSONObject();
        selector.put("strategy", chainTunnels.getFirst().getStrategy());
        selector.put("maxFails", 1);
        selector.put("failTimeout", 600000000000L); // 600 秒（纳秒单位）


        hop.put("selector", selector);
        hop.put("nodes", nodes);

        JSONArray hops = new JSONArray();
        hops.add(hop);

        JSONObject data = new JSONObject();
        data.put("name", "chains_" + chainTunnels.getFirst().getTunnelId());
        data.put("hops", hops);

        WebSocketServer.send_msg(node_id, data, "AddChains");
    }

    public static void DeleteChains(Long node_id, String name) {
        JSONObject data = new JSONObject();
        data.put("chain", name);
        WebSocketServer.send_msg(node_id, data, "DeleteChains");
    }

    public static void AddChainService(Long node_id, ChainTunnel chainTunnel, Map<Long, Node> node_s) {
        JSONArray services = new JSONArray();
        Node node_info = node_s.get(chainTunnel.getNodeId());
        JSONObject service_item = new JSONObject();
        service_item.put("name", chainTunnel.getTunnelId() + "_tls");
        service_item.put("addr", node_info.getTcpListenAddr() + ":" + chainTunnel.getPort());
        if (StringUtils.isNotBlank(node_info.getInterfaceName())) {
            JSONObject metadata = new JSONObject();
            metadata.put("interface", node_info.getInterfaceName());
            service_item.put("metadata", metadata);
        }

        JSONObject handler = new JSONObject();
        handler.put("type", "relay");
        if (chainTunnel.getChainType() == 2){
            handler.put("chain","chains_" + chainTunnel.getTunnelId());
        }
        service_item.put("handler", handler);

        JSONObject listener = new JSONObject();
        listener.put("type", chainTunnel.getProtocol());
        service_item.put("listener", listener);

        services.add(service_item);

        WebSocketServer.send_msg(node_id, services, "AddService");
    }

    public static void DeleteChainService(Long node_id, JSONArray services) {
        JSONObject data = new JSONObject();
        data.put("services", services);
        WebSocketServer.send_msg(node_id, data, "DeleteService");
    }

    public static void AddAndUpdateService(String name, Integer limiter, Node node, Forward forward, ForwardPort forwardPort, Tunnel tunnel, String meth) {
        JSONArray services = new JSONArray();
        String[] protocols = {"tcp", "udp"};
        for (String protocol : protocols) {
            JSONObject service = new JSONObject();
            service.put("name", name + "_" + protocol);
            if (Objects.equals(protocol, "tcp")){
                service.put("addr", node.getTcpListenAddr() + ":" + forwardPort.getPort());
            }else {
                service.put("addr", node.getUdpListenAddr() + ":" + forwardPort.getPort());
            }

            if (StringUtils.isNotBlank(node.getInterfaceName())) {
                JSONObject metadata = new JSONObject();
                metadata.put("interface", node.getInterfaceName());
                service.put("metadata", metadata);
            }

            // 添加限流器配置
            if (limiter != null) {
                service.put("limiter", limiter.toString());
            }

            // 配置处理器
            JSONObject handler = new JSONObject();
            handler.put("type", protocol);
            if (tunnel.getType() == 2){
                handler.put("chain", "chains_" + forward.getTunnelId());
            }
            service.put("handler", handler);

            // 配置监听器
            JSONObject listener = createListener(protocol);
            service.put("listener", listener);

            JSONObject forwarder = createForwarder(forward.getRemoteAddr(), forward.getStrategy());
            service.put("forwarder", forwarder);

            services.add(service);
        }
        WebSocketServer.send_msg(node.getId(), services, meth);
    }

    public static void DeleteService(Long node_id, JSONArray services) {
        JSONObject data = new JSONObject();
        data.put("services", services);
        WebSocketServer.send_msg(node_id, data, "DeleteService");
    }

    public static void PauseAndResumeService(Long node_id, String name, String meth) {
        JSONObject data = new JSONObject();
        JSONArray services = new JSONArray();
        services.add(name + "_tcp");
        services.add(name + "_udp");
        data.put("services", services);
        WebSocketServer.send_msg(node_id, data, meth);
    }


    private static JSONObject createLimiterData(Long name, String speed) {
        JSONObject data = new JSONObject();
        data.put("name", name.toString());
        JSONArray limits = new JSONArray();
        limits.add("$ " + speed + "MB " + speed + "MB");
        data.put("limits", limits);
        return data;
    }

    private static JSONObject createListener(String protocol) {
        JSONObject listener = new JSONObject();
        listener.put("type", protocol);
        if (Objects.equals(protocol, "udp")) {
            JSONObject metadata = new JSONObject();
            metadata.put("keepAlive", true);
            listener.put("metadata", metadata);
        }
        return listener;
    }

    private static JSONObject createForwarder(String remoteAddr, String strategy) {
        JSONObject forwarder = new JSONObject();
        JSONArray nodes = new JSONArray();

        String[] split = remoteAddr.split(",");
        int num = 1;
        for (String addr : split) {
            JSONObject node = new JSONObject();
            node.put("name", "node_" + num);
            node.put("addr", addr);
            nodes.add(node);
            num++;
        }

        if (strategy == null || strategy.equals("")) {
            strategy = "fifo";
        }

        forwarder.put("nodes", nodes);

        JSONObject selector = new JSONObject();
        selector.put("strategy", strategy);
        selector.put("maxFails", 1);
        selector.put("failTimeout", "600s");
        forwarder.put("selector", selector);
        return forwarder;
    }


}
