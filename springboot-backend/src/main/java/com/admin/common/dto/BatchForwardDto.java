package com.admin.common.dto;

import lombok.Data;
import java.util.List;

@Data
public class BatchForwardDto {
    private List<Long> ids;
    private Integer tunnelId;
}
