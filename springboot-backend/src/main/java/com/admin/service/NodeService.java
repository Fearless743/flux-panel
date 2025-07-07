package com.admin.service;

import com.admin.common.dto.NodeDto;
import com.admin.common.dto.NodeUpdateDto;
import com.admin.common.dto.PageDto;
import com.admin.common.lang.R;
import com.admin.entity.Node;
import com.baomidou.mybatisplus.extension.service.IService;

/**
 * <p>
 *  服务类
 * </p>
 *
 * @author QAQ
 * @since 2025-06-03
 */
public interface NodeService extends IService<Node> {

    R createNode(NodeDto nodeDto);

    R getAllNodes();

    R updateNode(NodeUpdateDto nodeUpdateDto);

    R deleteNode(Long id);

    Node getNodeById(Long id);

    R getInstallCommand(Long id);

    /**
     * 检查和修复节点状态
     * @param params 包含节点ID的参数（可选）
     * @return 检查结果
     */
    R checkAndFixNodeStatus(java.util.Map<String, Object> params);
}
