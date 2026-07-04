package com.admin.service.impl;

import com.admin.common.dto.*;

import com.admin.common.lang.R;
import com.admin.common.utils.GostUtil;
import com.admin.common.utils.JwtUtil;
import com.admin.common.utils.WebSocketServer;
import com.admin.entity.*;
import com.admin.mapper.TunnelMapper;
import com.admin.mapper.UserTunnelMapper;
import com.admin.service.*;
import com.alibaba.fastjson.JSONArray;
import com.alibaba.fastjson.JSONObject;
import com.baomidou.mybatisplus.core.conditions.query.QueryWrapper;
import com.baomidou.mybatisplus.extension.service.impl.ServiceImpl;
import lombok.Data;
import org.apache.commons.lang3.StringUtils;
import org.springframework.beans.BeanUtils;
import org.springframework.stereotype.Service;

import javax.annotation.Resource;
import java.util.*;
import java.util.stream.Collectors;

/**
 *
 * @author QAQ
 * @since 2025-06-03
 */
@Service
public class TunnelServiceImpl extends ServiceImpl<TunnelMapper, Tunnel> implements TunnelService {


    @Resource
    UserTunnelMapper userTunnelMapper;

    @Resource
    NodeService nodeService;

    @Resource
    ForwardService forwardService;

    @Resource
    UserTunnelService userTunnelService;

    @Resource
    ChainTunnelService chainTunnelService;

    @Resource
    ForwardPortService forwardPortService;


    @Override
    public R createTunnel(TunnelDto tunnelDto) {

        int count = this.count(new QueryWrapper<Tunnel>().eq("name", tunnelDto.getName()));
        if (count > 0) return R.err("隧道名称重复");
        if (tunnelDto.getType() == 2 && tunnelDto.getOutNodeId() == null) return R.err("出口不能为空");


        List<ChainTunnel> chainTunnels = new ArrayList<>();
        Map<Long, Node> nodes = new HashMap<>();

        List<Long> node_ids = new ArrayList<>();
        for (ChainTunnel in_node : tunnelDto.getInNodeId()) {
            node_ids.add(in_node.getNodeId());
            chainTunnels.add(in_node);

            Node node = nodeService.getById(in_node.getNodeId());
            if (node == null) return R.err("节点不存在");
            nodes.put(node.getId(), node);
        }

        if (tunnelDto.getType() == 2) {
            // 处理转发链节点，为每一跳设置inx
            int inx = 1;
            for (List<ChainTunnel> chainNode : tunnelDto.getChainNodes()) {
                for (ChainTunnel chain_node : chainNode) {
                    node_ids.add(chain_node.getNodeId());
                    Node node = nodeService.getById(chain_node.getNodeId());
                    if (node == null) return R.err("节点不存在");
                    nodes.put(node.getId(), node);
                    Integer nodePort = getNodePort(chain_node.getNodeId());
                    chain_node.setPort(nodePort);
                    chain_node.setInx(inx); // 设置转发链序号
                    chainTunnels.add(chain_node);
                }
                inx++; // 每一跳递增
            }
            for (ChainTunnel out_node : tunnelDto.getOutNodeId()) {
                node_ids.add(out_node.getNodeId());
                Node node = nodeService.getById(out_node.getNodeId());
                if (node == null) return R.err("节点不存在");
                nodes.put(node.getId(), node);
                Integer nodePort = getNodePort(out_node.getNodeId());
                out_node.setPort(nodePort);
                chainTunnels.add(out_node);
            }

        }
        Set<Long> set = new HashSet<>(node_ids);
        boolean hasDuplicate = set.size() != node_ids.size();
        if (hasDuplicate) return R.err("节点重复");

        List<Node> list = nodeService.list(new QueryWrapper<Node>().in("id", node_ids));
        if (list.size() != node_ids.size()) return R.err("部分节点不存在");
        for (Node node : list) {
            if (node.getStatus() != 1) return R.err("部分节点不在线");
        }


        Tunnel tunnel = new Tunnel();
        BeanUtils.copyProperties(tunnelDto, tunnel);
        tunnel.setStatus(1);
        long currentTime = System.currentTimeMillis();
        tunnel.setCreatedTime(currentTime);
        tunnel.setUpdatedTime(currentTime);
        if (StringUtils.isEmpty(tunnel.getInIp())){
            StringBuilder in_ip = new StringBuilder();
            for (ChainTunnel chainTunnel : tunnelDto.getInNodeId()) {
                Node node = nodes.get(chainTunnel.getNodeId());
                in_ip.append(node.getServerIp()).append(",");
            }
            in_ip.deleteCharAt(in_ip.length() - 1);
            tunnel.setInIp(in_ip.toString());
        }

        this.save(tunnel);
        for (ChainTunnel chainTunnel : chainTunnels) {
            chainTunnel.setTunnelId(tunnel.getId());
        }
        chainTunnelService.saveBatch(chainTunnels);

        List<JSONObject> chain_success = new ArrayList<>();
        List<JSONObject> service_success = new ArrayList<>();



        if (tunnel.getType() == 2) {

            for (ChainTunnel in_node : tunnelDto.getInNodeId()) {
                // 创建Chain， 指向chainNode的第一跳。如果chainNode为空就是指向出口
                if (tunnelDto.getChainNodes().isEmpty()) { // 指向出口
                    GostDto gostDto = GostUtil.AddChains(in_node.getNodeId(), tunnelDto.getOutNodeId(), nodes);
                    isError(gostDto);

                } else {
                    GostDto gostDto = GostUtil.AddChains(in_node.getNodeId(), tunnelDto.getChainNodes().getFirst(), nodes);// 指向第一跳
                    if (Objects.equals(gostDto.getMsg(), "OK")){
                        JSONObject data = new JSONObject();
                        data.put("node_id", in_node.getNodeId());
                        data.put("name", "chains_" + tunnel.getId());
                        chain_success.add(data);
                    }else {
                        this.removeById(tunnel.getId());
                        chainTunnelService.remove(new QueryWrapper<ChainTunnel>().eq("tunnel_id", tunnel.getId()));
                        for (JSONObject chainSuccess : chain_success) {
                            GostDto deleteChains = GostUtil.DeleteChains(chainSuccess.getLong("node_id"), chainSuccess.getString("name"));
                            System.out.println(deleteChains);
                        }
                        return R.err(gostDto.getMsg());
                    }
                }
            }

            for (int i = 0; i < tunnelDto.getChainNodes().size(); i++) {
                //  创建Chain和Service。每一条的Chain都是指向下一跳。最后一跳指向出口， Service是监听端口
                List<ChainTunnel> chainTunnels1 = tunnelDto.getChainNodes().get(i);
                for (ChainTunnel chainTunnel : chainTunnels1) {
                    int inx = i+1;
                    if (inx >= tunnelDto.getChainNodes().size()) { // 指向出口
                        GostDto gostDto = GostUtil.AddChains(chainTunnel.getNodeId(), tunnelDto.getOutNodeId(), nodes);
                        if (Objects.equals(gostDto.getMsg(), "OK")){
                            JSONObject data = new JSONObject();
                            data.put("node_id", chainTunnel.getNodeId());
                            data.put("name", "chains_" + tunnel.getId());
                            chain_success.add(data);
                        }else {
                            this.removeById(tunnel.getId());
                            chainTunnelService.remove(new QueryWrapper<ChainTunnel>().eq("tunnel_id", tunnel.getId()));
                            for (JSONObject chainSuccess : chain_success) {
                                GostDto deleteChains = GostUtil.DeleteChains(chainSuccess.getLong("node_id"), chainSuccess.getString("name"));
                                System.out.println(deleteChains);
                            }
                            return R.err(gostDto.getMsg());
                        }
                    } else {
                        GostDto gostDto = GostUtil.AddChains(chainTunnel.getNodeId(), tunnelDto.getChainNodes().get(inx), nodes);
                        if (Objects.equals(gostDto.getMsg(), "OK")){
                            JSONObject data = new JSONObject();
                            data.put("node_id", chainTunnel.getNodeId());
                            data.put("name", "chains_" + tunnel.getId());
                            chain_success.add(data);
                        }else {
                            this.removeById(tunnel.getId());
                            chainTunnelService.remove(new QueryWrapper<ChainTunnel>().eq("tunnel_id", tunnel.getId()));
                            for (JSONObject chainSuccess : chain_success) {
                                GostDto deleteChains = GostUtil.DeleteChains(chainSuccess.getLong("node_id"), chainSuccess.getString("name"));
                                System.out.println(deleteChains);
                            }
                            return R.err(gostDto.getMsg());
                        }
                    }

                    GostDto gostDto = GostUtil.AddChainService(chainTunnel.getNodeId(), chainTunnel, nodes);
                    if (Objects.equals(gostDto.getMsg(), "OK")){
                        JSONObject data = new JSONObject();
                        data.put("node_id", chainTunnel.getNodeId());
                        data.put("name", tunnel.getId() + "_tls");
                        service_success.add(data);
                    }else {
                        this.removeById(tunnel.getId());
                        chainTunnelService.remove(new QueryWrapper<ChainTunnel>().eq("tunnel_id", tunnel.getId()));
                        for (JSONObject serviceSuccess : service_success) {
                            JSONArray jsonArray = new JSONArray();
                            jsonArray.add(serviceSuccess.getString("name"));
                            GostDto deleteService = GostUtil.DeleteService(serviceSuccess.getLong("node_id"), jsonArray);
                            System.out.println(deleteService);
                        }
                        return R.err(gostDto.getMsg());
                    }
                }

            }


            for (ChainTunnel out_node : tunnelDto.getOutNodeId()) {
                GostDto gostDto = GostUtil.AddChainService(out_node.getNodeId(), out_node, nodes);
                if (Objects.equals(gostDto.getMsg(), "OK")){
                    JSONObject data = new JSONObject();
                    data.put("node_id", out_node.getNodeId());
                    data.put("name", tunnel.getId() + "_tls");
                    service_success.add(data);
                }else {
                    this.removeById(tunnel.getId());
                    chainTunnelService.remove(new QueryWrapper<ChainTunnel>().eq("tunnel_id", tunnel.getId()));
                    for (JSONObject serviceSuccess : service_success) {
                        JSONArray jsonArray = new JSONArray();
                        jsonArray.add(serviceSuccess.getString("name"));
                        GostDto deleteService = GostUtil.DeleteService(serviceSuccess.getLong("node_id"), jsonArray);
                        System.out.println(deleteService);
                    }
                    return R.err(gostDto.getMsg());
                }
            }

        }
        return R.ok();
    }


    @Override
    public R getAllTunnels() {
        List<Tunnel> tunnelList = this.list();
        
        // 查询所有隧道的ChainTunnel信息
        List<Long> tunnelIds = tunnelList.stream()
                .map(Tunnel::getId)
                .collect(Collectors.toList());
        
        if (tunnelIds.isEmpty()) {
            return R.ok(new ArrayList<TunnelDetailDto>());
        }
        
        // 批量查询所有ChainTunnel记录
        List<ChainTunnel> allChainTunnels = chainTunnelService.list(
                new QueryWrapper<ChainTunnel>().in("tunnel_id", tunnelIds)
        );
        
        // 按tunnelId分组
        Map<Long, List<ChainTunnel>> chainTunnelMap = allChainTunnels.stream()
                .collect(Collectors.groupingBy(ChainTunnel::getTunnelId));
        
        // 转换为TunnelDetailDto列表
        List<TunnelDetailDto> detailDtoList = tunnelList.stream()
                .map(tunnel -> {
                    TunnelDetailDto detailDto = new TunnelDetailDto();
                    BeanUtils.copyProperties(tunnel, detailDto);
                    
                    List<ChainTunnel> chainTunnels = chainTunnelMap.getOrDefault(tunnel.getId(), new ArrayList<>());
                    
                    // 按chainType分类节点
                    // 入口节点 (chainType = 1)
                    List<ChainTunnel> inNodes = chainTunnels.stream()
                            .filter(ct -> ct.getChainType() != null && ct.getChainType() == 1)
                            .collect(Collectors.toList());
                    detailDto.setInNodeId(inNodes);

                    detailDto.setInIp(tunnel.getInIp());
                    
                    // 转发链节点 (chainType = 2) - 按inx分组
                    Map<Integer, List<ChainTunnel>> chainNodesMap = chainTunnels.stream()
                            .filter(ct -> ct.getChainType() != null && ct.getChainType() == 2)
                            .collect(Collectors.groupingBy(
                                    ct -> ct.getInx() != null ? ct.getInx() : 0,
                                    Collectors.toList()
                            ));
                    
                    // 将Map转换为按inx排序的二维列表
                    List<List<ChainTunnel>> chainNodesList = chainNodesMap.entrySet().stream()
                            .sorted(Map.Entry.comparingByKey())
                            .map(Map.Entry::getValue)
                            .collect(Collectors.toList());
                    detailDto.setChainNodes(chainNodesList);
                    
                    // 出口节点 (chainType = 3)
                    List<ChainTunnel> outNodes = chainTunnels.stream()
                            .filter(ct -> ct.getChainType() != null && ct.getChainType() == 3)
                            .collect(Collectors.toList());
                    detailDto.setOutNodeId(outNodes);
                    
                    return detailDto;
                })
                .collect(Collectors.toList());
        
        return R.ok(detailDtoList);
    }


    @Override
    public R updateTunnel(TunnelUpdateDto tunnelUpdateDto) {
        Tunnel existingTunnel = this.getById(tunnelUpdateDto.getId());
        if (existingTunnel == null) return R.err("隧道不存在");

        // 传入入口节点时，重建隧道节点拓扑并实时下发gost配置
        if (tunnelUpdateDto.getInNodeId() != null && !tunnelUpdateDto.getInNodeId().isEmpty()) {
            R result = reconfigureTunnelNodes(existingTunnel, tunnelUpdateDto);
            if (result.getCode() != 0) return result;
        }

        Tunnel tunnel = new Tunnel();
        tunnel.setId(tunnelUpdateDto.getId());
        tunnel.setName(tunnelUpdateDto.getName());
        tunnel.setFlow(tunnelUpdateDto.getFlow());
        tunnel.setTrafficRatio(tunnelUpdateDto.getTrafficRatio());
        tunnel.setInIp(tunnelUpdateDto.getInIp());

        if (StringUtils.isEmpty(tunnel.getInIp())){
            StringBuilder in_ip = new StringBuilder();
            List<ChainTunnel> chainTunnels = chainTunnelService.list(new QueryWrapper<ChainTunnel>().eq("tunnel_id", tunnel.getId()).eq("chain_type", 1));
            for (ChainTunnel chainTunnel : chainTunnels) {
                Node node = nodeService.getById(chainTunnel.getNodeId());
                if (node == null)return R.err("隧道节点数据错误，部分节点不存在");
                in_ip.append(node.getServerIp()).append(",");
            }
            in_ip.deleteCharAt(in_ip.length() - 1);
            tunnel.setInIp(in_ip.toString());
        }

        this.updateById(tunnel);
        return R.ok();
    }

    /**
     * 重建隧道的入口/转发链/出口节点配置，并将差异实时下发到各节点端gost
     */
    private R reconfigureTunnelNodes(Tunnel tunnel, TunnelUpdateDto dto) {
        Long tunnelId = tunnel.getId();
        boolean isTunnelForward = tunnel.getType() == 2;

        List<List<ChainTunnel>> chainGroups = new ArrayList<>();
        List<ChainTunnel> outNodesReq = new ArrayList<>();
        if (isTunnelForward) {
            if (dto.getOutNodeId() == null || dto.getOutNodeId().isEmpty()) return R.err("出口不能为空");
            if (dto.getChainNodes() != null) chainGroups = dto.getChainNodes();
            outNodesReq = dto.getOutNodeId();
        }

        // 规范化新配置（不信任前端传来的id/port，仅取拓扑信息）
        List<ChainTunnel> newEntries = new ArrayList<>();
        List<List<ChainTunnel>> newChains = new ArrayList<>();
        List<ChainTunnel> newOuts = new ArrayList<>();
        List<Long> nodeIds = new ArrayList<>();

        for (ChainTunnel in : dto.getInNodeId()) {
            ChainTunnel ct = new ChainTunnel();
            ct.setTunnelId(tunnelId);
            ct.setChainType(1);
            ct.setNodeId(in.getNodeId());
            newEntries.add(ct);
            nodeIds.add(in.getNodeId());
        }
        int inx = 1;
        for (List<ChainTunnel> group : chainGroups) {
            List<ChainTunnel> newGroup = new ArrayList<>();
            for (ChainTunnel c : group) {
                ChainTunnel ct = new ChainTunnel();
                ct.setTunnelId(tunnelId);
                ct.setChainType(2);
                ct.setNodeId(c.getNodeId());
                ct.setProtocol(c.getProtocol());
                ct.setStrategy(c.getStrategy());
                ct.setInx(inx);
                newGroup.add(ct);
                nodeIds.add(c.getNodeId());
            }
            if (!newGroup.isEmpty()) {
                newChains.add(newGroup);
                inx++;
            }
        }
        for (ChainTunnel out : outNodesReq) {
            ChainTunnel ct = new ChainTunnel();
            ct.setTunnelId(tunnelId);
            ct.setChainType(3);
            ct.setNodeId(out.getNodeId());
            ct.setProtocol(out.getProtocol());
            ct.setStrategy(out.getStrategy());
            newOuts.add(ct);
            nodeIds.add(out.getNodeId());
        }

        Set<Long> nodeIdSet = new HashSet<>(nodeIds);
        if (nodeIdSet.size() != nodeIds.size()) return R.err("节点重复");

        Map<Long, Node> nodes = new HashMap<>();
        List<Node> nodeList = nodeService.list(new QueryWrapper<Node>().in("id", nodeIds));
        if (nodeList.size() != nodeIdSet.size()) return R.err("部分节点不存在");
        for (Node node : nodeList) {
            if (node.getStatus() != 1) return R.err("节点 " + node.getName() + " 不在线，无法下发配置");
            nodes.put(node.getId(), node);
        }

        // 旧配置记录
        List<ChainTunnel> oldRecords = chainTunnelService.list(new QueryWrapper<ChainTunnel>().eq("tunnel_id", tunnelId));
        Map<Long, Integer> oldPorts = new HashMap<>();
        for (ChainTunnel old : oldRecords) {
            if (old.getPort() != null) oldPorts.put(old.getNodeId(), old.getPort());
        }

        // 为转发链/出口节点分配端口，同一节点沿用旧端口避免端口漂移
        try {
            for (List<ChainTunnel> group : newChains) {
                for (ChainTunnel ct : group) {
                    Integer port = oldPorts.get(ct.getNodeId());
                    ct.setPort(port != null ? port : getNodePort(ct.getNodeId()));
                }
            }
            for (ChainTunnel ct : newOuts) {
                Integer port = oldPorts.get(ct.getNodeId());
                ct.setPort(port != null ? port : getNodePort(ct.getNodeId()));
            }
        } catch (RuntimeException e) {
            return R.err(e.getMessage());
        }

        List<String> failures = new ArrayList<>();

        // 清理不再需要的旧gost配置
        if (isTunnelForward) {
            Set<Long> chainNeeded = new HashSet<>();   // 需要 chains_x 的节点：入口 + 转发链
            Set<Long> serviceNeeded = new HashSet<>(); // 需要 x_tls 服务的节点：转发链 + 出口
            for (ChainTunnel ct : newEntries) chainNeeded.add(ct.getNodeId());
            for (List<ChainTunnel> group : newChains) {
                for (ChainTunnel ct : group) {
                    chainNeeded.add(ct.getNodeId());
                    serviceNeeded.add(ct.getNodeId());
                }
            }
            for (ChainTunnel ct : newOuts) serviceNeeded.add(ct.getNodeId());

            Set<Long> chainCleaned = new HashSet<>();
            Set<Long> serviceCleaned = new HashSet<>();
            for (ChainTunnel old : oldRecords) {
                boolean hadChain = old.getChainType() == 1 || old.getChainType() == 2;
                boolean hadService = old.getChainType() == 2 || old.getChainType() == 3;
                if (hadChain && !chainNeeded.contains(old.getNodeId()) && chainCleaned.add(old.getNodeId())) {
                    GostUtil.DeleteChains(old.getNodeId(), "chains_" + tunnelId);
                }
                if (hadService && !serviceNeeded.contains(old.getNodeId()) && serviceCleaned.add(old.getNodeId())) {
                    JSONArray services = new JSONArray();
                    services.add(tunnelId + "_tls");
                    GostUtil.DeleteService(old.getNodeId(), services);
                }
            }
        }

        // 替换数据库记录
        chainTunnelService.remove(new QueryWrapper<ChainTunnel>().eq("tunnel_id", tunnelId));
        List<ChainTunnel> allNew = new ArrayList<>(newEntries);
        for (List<ChainTunnel> group : newChains) allNew.addAll(group);
        allNew.addAll(newOuts);
        chainTunnelService.saveBatch(allNew);

        // 下发新配置（先更新，不存在则新增）
        if (isTunnelForward) {
            for (ChainTunnel entry : newEntries) {
                List<ChainTunnel> target = newChains.isEmpty() ? newOuts : newChains.getFirst();
                GostDto res = pushChains(entry.getNodeId(), target, nodes);
                if (!Objects.equals(res.getMsg(), "OK")) {
                    failures.add("入口[" + nodes.get(entry.getNodeId()).getName() + "]链下发失败: " + res.getMsg());
                }
            }
            for (int i = 0; i < newChains.size(); i++) {
                List<ChainTunnel> target = (i + 1 < newChains.size()) ? newChains.get(i + 1) : newOuts;
                for (ChainTunnel ct : newChains.get(i)) {
                    GostDto res = pushChains(ct.getNodeId(), target, nodes);
                    if (!Objects.equals(res.getMsg(), "OK")) {
                        failures.add("转发链[" + nodes.get(ct.getNodeId()).getName() + "]链下发失败: " + res.getMsg());
                    }
                    res = pushChainService(ct.getNodeId(), ct, nodes);
                    if (!Objects.equals(res.getMsg(), "OK")) {
                        failures.add("转发链[" + nodes.get(ct.getNodeId()).getName() + "]服务下发失败: " + res.getMsg());
                    }
                }
            }
            for (ChainTunnel ct : newOuts) {
                GostDto res = pushChainService(ct.getNodeId(), ct, nodes);
                if (!Objects.equals(res.getMsg(), "OK")) {
                    failures.add("出口[" + nodes.get(ct.getNodeId()).getName() + "]服务下发失败: " + res.getMsg());
                }
            }
        }

        // 入口节点变化时迁移其上的转发服务
        Set<Long> oldEntryIds = oldRecords.stream()
                .filter(ct -> ct.getChainType() != null && ct.getChainType() == 1)
                .map(ChainTunnel::getNodeId)
                .collect(Collectors.toSet());
        Set<Long> newEntryIds = newEntries.stream().map(ChainTunnel::getNodeId).collect(Collectors.toSet());
        migrateForwardServices(tunnel, oldEntryIds, newEntryIds, nodes, failures);

        if (!failures.isEmpty()) {
            return R.err("配置已保存，但部分下发失败: " + String.join("；", failures));
        }
        return R.ok();
    }

    private GostDto pushChains(Long nodeId, List<ChainTunnel> target, Map<Long, Node> nodes) {
        GostDto res = GostUtil.UpdateChains(nodeId, target, nodes);
        if (!Objects.equals(res.getMsg(), "OK") && res.getMsg() != null && res.getMsg().contains("not found")) {
            res = GostUtil.AddChains(nodeId, target, nodes);
        }
        return res;
    }

    private GostDto pushChainService(Long nodeId, ChainTunnel chainTunnel, Map<Long, Node> nodes) {
        GostDto res = GostUtil.AddChainService(nodeId, chainTunnel, nodes, "UpdateService");
        if (!Objects.equals(res.getMsg(), "OK") && res.getMsg() != null && res.getMsg().contains("not found")) {
            res = GostUtil.AddChainService(nodeId, chainTunnel, nodes, "AddService");
        }
        return res;
    }

    /**
     * 入口节点变化时，把隧道下所有转发的入口服务从移除的节点上删除、在新增的节点上创建
     */
    private void migrateForwardServices(Tunnel tunnel, Set<Long> oldEntryIds, Set<Long> newEntryIds,
                                        Map<Long, Node> nodes, List<String> failures) {
        Set<Long> removed = new HashSet<>(oldEntryIds);
        removed.removeAll(newEntryIds);
        Set<Long> added = new HashSet<>(newEntryIds);
        added.removeAll(oldEntryIds);
        if (removed.isEmpty() && added.isEmpty()) return;

        List<Forward> forwards = forwardService.list(new QueryWrapper<Forward>().eq("tunnel_id", tunnel.getId()));
        for (Forward forward : forwards) {
            UserTunnel userTunnel = userTunnelService.getOne(new QueryWrapper<UserTunnel>()
                    .eq("user_id", forward.getUserId())
                    .eq("tunnel_id", tunnel.getId()));
            String serviceName = forward.getId() + "_" + forward.getUserId() + "_" + (userTunnel != null ? userTunnel.getId() : 0);
            Integer limiter = userTunnel != null ? userTunnel.getSpeedId() : null;

            // 记录转发原入口端口（优先取保留节点上的端口），迁移到新节点时尽量沿用
            Integer preferredPort = null;
            List<ForwardPort> existingPorts = forwardPortService.list(
                    new QueryWrapper<ForwardPort>().eq("forward_id", forward.getId()));
            for (ForwardPort fp : existingPorts) {
                if (fp.getPort() == null) continue;
                if (preferredPort == null) preferredPort = fp.getPort();
                if (!removed.contains(fp.getNodeId())) {
                    preferredPort = fp.getPort();
                    break;
                }
            }

            for (Long nodeId : removed) {
                JSONArray services = new JSONArray();
                services.add(serviceName + "_tcp");
                services.add(serviceName + "_udp");
                GostUtil.DeleteService(nodeId, services);
                forwardPortService.remove(new QueryWrapper<ForwardPort>()
                        .eq("forward_id", forward.getId())
                        .eq("node_id", nodeId));
            }

            for (Long nodeId : added) {
                try {
                    List<Integer> availablePorts = getAvailablePorts(nodeId);
                    if (availablePorts.isEmpty()) {
                        throw new RuntimeException("节点端口已满，无可用端口");
                    }
                    // 原入口端口在新节点可用时保持不变，否则回退自动分配
                    Integer port = (preferredPort != null && availablePorts.contains(preferredPort))
                            ? preferredPort : availablePorts.getFirst();
                    ForwardPort forwardPort = new ForwardPort();
                    forwardPort.setForwardId(forward.getId());
                    forwardPort.setNodeId(nodeId);
                    forwardPort.setPort(port);
                    forwardPortService.save(forwardPort);
                    GostDto res = GostUtil.AddAndUpdateService(serviceName, limiter, nodes.get(nodeId), forward, forwardPort, tunnel, "AddService");
                    if (!Objects.equals(res.getMsg(), "OK")) {
                        failures.add("转发[" + forward.getName() + "]在节点[" + nodes.get(nodeId).getName() + "]创建失败: " + res.getMsg());
                    } else if (forward.getStatus() != null && forward.getStatus() == 0) {
                        // 已暂停的转发保持暂停状态
                        GostUtil.PauseAndResumeService(nodeId, serviceName, "PauseService");
                    }
                } catch (RuntimeException e) {
                    failures.add("转发[" + forward.getName() + "]在节点[" + nodes.get(nodeId).getName() + "]分配端口失败: " + e.getMessage());
                }
            }
        }
    }


    @Override
    public R deleteTunnel(Long id) {
        Tunnel tunnel = this.getById(id);
        if (tunnel == null) return R.err("隧道不存在");
        List<Forward> forwardList = forwardService.list(new QueryWrapper<Forward>().eq("tunnel_id", id));
        for (Forward forward : forwardList) {
            forwardService.deleteForward(forward.getId());
        }
        forwardService.remove(new QueryWrapper<Forward>().eq("tunnel_id", id));
        userTunnelService.remove(new QueryWrapper<UserTunnel>().eq("tunnel_id", id));
        this.removeById(id);

        List<ChainTunnel> chainTunnels = chainTunnelService.list(new QueryWrapper<ChainTunnel>().eq("tunnel_id", id));
        for (ChainTunnel chainTunnel : chainTunnels) {
            if (chainTunnel.getChainType() == 1){ // 入口
                GostUtil.DeleteChains(chainTunnel.getNodeId(), "chains_" + chainTunnel.getTunnelId());
            }
            else if (chainTunnel.getChainType() == 2){ // 链
                GostUtil.DeleteChains(chainTunnel.getNodeId(), "chains_" + chainTunnel.getTunnelId());
                JSONArray services = new JSONArray();
                services.add(chainTunnel.getTunnelId() + "_tls");
                GostUtil.DeleteService(chainTunnel.getNodeId(), services);
            }
            else { // 出口
                JSONArray services = new JSONArray();
                services.add(chainTunnel.getTunnelId() + "_tls");
                GostUtil.DeleteService(chainTunnel.getNodeId(), services);
            }
        }
        chainTunnelService.remove(new QueryWrapper<ChainTunnel>().eq("tunnel_id", id));
        return R.ok();
    }


    @Override
    public R userTunnel() {
        List<Tunnel> tunnelEntities;
        Integer roleId = JwtUtil.getRoleIdFromToken();
        Integer userId = JwtUtil.getUserIdFromToken();
        if (roleId == 0) {
            tunnelEntities = this.list(new QueryWrapper<Tunnel>().eq("status", 1));
        } else {
            tunnelEntities = java.util.Collections.emptyList(); // 返回空列表
            List<UserTunnel> userTunnels = userTunnelMapper.selectList(
                    new QueryWrapper<UserTunnel>().eq("user_id", userId)
            );
            if (!userTunnels.isEmpty()) {
                List<Integer> tunnelIds = userTunnels.stream()
                        .map(UserTunnel::getTunnelId)
                        .collect(Collectors.toList());
                tunnelEntities = this.list(new QueryWrapper<Tunnel>()
                        .in("id", tunnelIds)
                        .eq("status", 1));
            }

        }
        return R.ok(tunnelEntities);
    }


    @Override
    public R diagnoseTunnel(Long tunnelId) {
        Tunnel tunnel = this.getById(tunnelId);
        if (tunnel == null) {
            return R.err("隧道不存在");
        }

        List<ChainTunnel> chainTunnels = chainTunnelService.list(
                new QueryWrapper<ChainTunnel>().eq("tunnel_id", tunnelId)
        );

        if (chainTunnels.isEmpty()) {
            return R.err("隧道配置不完整");
        }

        List<ChainTunnel> inNodes = chainTunnels.stream()
                .filter(ct -> ct.getChainType() == 1)
                .toList();

        Map<Integer, List<ChainTunnel>> chainNodesMap = chainTunnels.stream()
                .filter(ct -> ct.getChainType() == 2)
                .collect(Collectors.groupingBy(
                        ct -> ct.getInx() != null ? ct.getInx() : 0,
                        Collectors.toList()
                ));

        List<List<ChainTunnel>> chainNodesList = chainNodesMap.entrySet().stream()
                .sorted(Map.Entry.comparingByKey())
                .map(Map.Entry::getValue)
                .toList();

        List<ChainTunnel> outNodes = chainTunnels.stream()
                .filter(ct -> ct.getChainType() == 3)
                .toList();

        List<DiagnosisResult> results = new ArrayList<>();

        if (tunnel.getType() == 1) {
            for (ChainTunnel inNode : inNodes) {
                Node node = nodeService.getById(inNode.getNodeId());
                if (node != null) {
                    DiagnosisResult result = performTcpPingDiagnosisWithConnectionCheck(
                            node, "www.google.com", 443, "入口(" + node.getName() + ")->外网"
                    );
                    result.setFromChainType(1); // 入口
                    results.add(result);
                }
            }
        } else if (tunnel.getType() == 2) {
            for (ChainTunnel inNode : inNodes) {
                Node fromNode = nodeService.getById(inNode.getNodeId());
                
                if (fromNode != null) {
                    if (!chainNodesList.isEmpty()) {
                        for (ChainTunnel firstChainNode : chainNodesList.getFirst()) {
                            Node toNode = nodeService.getById(firstChainNode.getNodeId());
                            if (toNode != null) {
                                DiagnosisResult result = performTcpPingDiagnosisWithConnectionCheck(
                                        fromNode, toNode.getServerIp(), firstChainNode.getPort(),
                                        "入口(" + fromNode.getName() + ")->第1跳(" + toNode.getName() + ")"
                                );
                                result.setFromChainType(1); // 入口
                                result.setToChainType(2); // 链
                                result.setToInx(firstChainNode.getInx());
                                results.add(result);
                            }
                        }
                    } else if (!outNodes.isEmpty()) {
                        for (ChainTunnel outNode : outNodes) {
                            Node toNode = nodeService.getById(outNode.getNodeId());
                            if (toNode != null) {
                                DiagnosisResult result = performTcpPingDiagnosisWithConnectionCheck(
                                        fromNode, toNode.getServerIp(), outNode.getPort(),
                                        "入口(" + fromNode.getName() + ")->出口(" + toNode.getName() + ")"
                                );
                                result.setFromChainType(1);
                                result.setToChainType(3);
                                results.add(result);
                            }
                        }
                    }
                }
            }

            for (int i = 0; i < chainNodesList.size(); i++) {
                List<ChainTunnel> currentHop = chainNodesList.get(i);
                
                for (ChainTunnel currentNode : currentHop) {
                    Node fromNode = nodeService.getById(currentNode.getNodeId());
                    
                    if (fromNode != null) {
                        if (i + 1 < chainNodesList.size()) {
                            for (ChainTunnel nextNode : chainNodesList.get(i + 1)) {
                                Node toNode = nodeService.getById(nextNode.getNodeId());
                                if (toNode != null) {
                                    DiagnosisResult result = performTcpPingDiagnosisWithConnectionCheck(
                                            fromNode, toNode.getServerIp(), nextNode.getPort(),
                                            "第" + (i + 1) + "跳(" + fromNode.getName() + ")->第" + (i + 2) + "跳(" + toNode.getName() + ")"
                                    );
                                    result.setFromChainType(2);
                                    result.setFromInx(currentNode.getInx());
                                    result.setToChainType(2);
                                    result.setToInx(nextNode.getInx());
                                    results.add(result);
                                }
                            }
                        } else if (!outNodes.isEmpty()) {
                            for (ChainTunnel outNode : outNodes) {
                                Node toNode = nodeService.getById(outNode.getNodeId());
                                if (toNode != null) {
                                    DiagnosisResult result = performTcpPingDiagnosisWithConnectionCheck(
                                            fromNode, toNode.getServerIp(), outNode.getPort(),
                                            "第" + (i + 1) + "跳(" + fromNode.getName() + ")->出口(" + toNode.getName() + ")"
                                    );
                                    result.setFromChainType(2);
                                    result.setFromInx(currentNode.getInx());
                                    result.setToChainType(3);
                                    results.add(result);
                                }
                            }
                        }
                    }
                }
            }
            for (ChainTunnel outNode : outNodes) {
                Node node = nodeService.getById(outNode.getNodeId());
                if (node != null) {
                    DiagnosisResult result = performTcpPingDiagnosisWithConnectionCheck(
                            node, "www.google.com", 443, "出口(" + node.getName() + ")->外网"
                    );
                    result.setFromChainType(3);
                    results.add(result);
                }
            }
        }

        Map<String, Object> diagnosisReport = new HashMap<>();
        diagnosisReport.put("tunnelId", tunnelId);
        diagnosisReport.put("tunnelName", tunnel.getName());
        diagnosisReport.put("tunnelType", tunnel.getType() == 1 ? "端口转发" : "隧道转发");
        diagnosisReport.put("results", results);
        diagnosisReport.put("timestamp", System.currentTimeMillis());

        return R.ok(diagnosisReport);
    }

    public Integer getNodePort(Long nodeId) {
        List<Integer> availablePorts = getAvailablePorts(nodeId);
        if (availablePorts.isEmpty()) {
            throw new RuntimeException("节点端口已满，无可用端口");
        }
        return availablePorts.getFirst();
    }

    public List<Integer> getAvailablePorts(Long nodeId) {

        Node node = nodeService.getById(nodeId);
        if (node == null){
            throw new RuntimeException("节点不存在");
        }

        // 1. 查询隧道转发链占用的端口
        List<ChainTunnel> chainTunnels = chainTunnelService.list(
                new QueryWrapper<ChainTunnel>().eq("node_id", nodeId)
        );
        Set<Integer> usedPorts = chainTunnels.stream()
                .map(ChainTunnel::getPort)
                .filter(Objects::nonNull)
                .collect(Collectors.toSet());


        List<ForwardPort> list = forwardPortService.list(new QueryWrapper<ForwardPort>().eq("node_id", nodeId));
        Set<Integer> forwardUsedPorts = new HashSet<>();
        for (ForwardPort forwardPort : list) {
            forwardUsedPorts.add(forwardPort.getPort());
        }
        usedPorts.addAll(forwardUsedPorts);

        // 3. 从可用端口范围中筛选未被占用的端口
        List<Integer> parsedPorts = parsePorts(node.getPort());
        return parsedPorts.stream()
                .filter(p -> !usedPorts.contains(p))
                .toList();
    }

    public static List<Integer> parsePorts(String input) {
        Set<Integer> set = new HashSet<>();
        String[] parts = input.split(",");
        for (String part : parts) {
            part = part.trim();
            if (part.contains("-")) {
                String[] range = part.split("-");
                int start = Integer.parseInt(range[0]);
                int end = Integer.parseInt(range[1]);
                for (int i = start; i <= end; i++) {
                    set.add(i);
                }
            } else {
                set.add(Integer.parseInt(part));
            }
        }
        return set.stream().sorted().collect(Collectors.toList());
    }

    private void isError(GostDto gostDto){

    }

    private DiagnosisResult performTcpPingDiagnosis(Node node, String targetIp, int port, String description) {
        try {
            // 构建TCP ping请求数据
            JSONObject tcpPingData = new JSONObject();
            tcpPingData.put("ip", targetIp);
            tcpPingData.put("port", port);
            tcpPingData.put("count", 4);
            tcpPingData.put("timeout", 5000); // 5秒超时

            // 发送TCP ping命令到节点
            GostDto gostResult = WebSocketServer.send_msg(node.getId(), tcpPingData, "TcpPing");

            DiagnosisResult result = new DiagnosisResult();
            result.setNodeId(node.getId());
            result.setNodeName(node.getName());
            result.setTargetIp(targetIp);
            result.setTargetPort(port);
            result.setDescription(description);
            result.setTimestamp(System.currentTimeMillis());

            if (gostResult != null && "OK".equals(gostResult.getMsg())) {
                // 尝试解析TCP ping响应数据
                try {
                    if (gostResult.getData() != null) {
                        JSONObject tcpPingResponse = (JSONObject) gostResult.getData();
                        boolean success = tcpPingResponse.getBooleanValue("success");

                        result.setSuccess(success);
                        if (success) {
                            result.setMessage("TCP连接成功");
                            result.setAverageTime(tcpPingResponse.getDoubleValue("averageTime"));
                            result.setPacketLoss(tcpPingResponse.getDoubleValue("packetLoss"));
                        } else {
                            result.setMessage(tcpPingResponse.getString("errorMessage"));
                            result.setAverageTime(-1.0);
                            result.setPacketLoss(100.0);
                        }
                    } else {
                        // 没有详细数据，使用默认值
                        result.setSuccess(true);
                        result.setMessage("TCP连接成功");
                        result.setAverageTime(0.0);
                        result.setPacketLoss(0.0);
                    }
                } catch (Exception e) {
                    // 解析响应数据失败，但TCP ping命令本身成功了
                    result.setSuccess(true);
                    result.setMessage("TCP连接成功，但无法解析详细数据");
                    result.setAverageTime(0.0);
                    result.setPacketLoss(0.0);
                }
            } else {
                result.setSuccess(false);
                result.setMessage(gostResult != null ? gostResult.getMsg() : "节点无响应");
                result.setAverageTime(-1.0);
                result.setPacketLoss(100.0);
            }

            return result;
        } catch (Exception e) {
            DiagnosisResult result = new DiagnosisResult();
            result.setNodeId(node.getId());
            result.setNodeName(node.getName());
            result.setTargetIp(targetIp);
            result.setTargetPort(port);
            result.setDescription(description);
            result.setSuccess(false);
            result.setMessage("诊断执行异常: " + e.getMessage());
            result.setTimestamp(System.currentTimeMillis());
            result.setAverageTime(-1.0);
            result.setPacketLoss(100.0);
            return result;
        }
    }

    private DiagnosisResult performTcpPingDiagnosisWithConnectionCheck(Node node, String targetIp, int port, String description) {
        DiagnosisResult result = new DiagnosisResult();
        result.setNodeId(node.getId());
        result.setNodeName(node.getName());
        result.setTargetIp(targetIp);
        result.setTargetPort(port);
        result.setDescription(description);
        result.setTimestamp(System.currentTimeMillis());

        try {
            return performTcpPingDiagnosis(node, targetIp, port, description);
        } catch (Exception e) {
            result.setSuccess(false);
            result.setMessage("连接检查异常: " + e.getMessage());
            result.setAverageTime(-1.0);
            result.setPacketLoss(100.0);
            return result;
        }
    }


}
