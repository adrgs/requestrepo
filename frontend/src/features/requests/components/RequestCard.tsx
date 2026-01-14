import { memo } from "react";
import { Card, CardBody, Chip } from "@heroui/react";
import { cn, formatRelativeTime, getMethodColor } from "@/lib/utils";
import type { Request } from "@/types";
import { isHttpRequest } from "@/types";

interface RequestCardProps {
  request: Request;
  isActive: boolean;
  isVisited: boolean;
  onSelect: () => void;
}

export const RequestCard = memo(function RequestCard({
  request,
  isActive,
  isVisited,
  onSelect,
}: RequestCardProps) {
  const isHttp = isHttpRequest(request);

  return (
    <Card
      isPressable
      onPress={onSelect}
      className={cn(
        "w-full transition-all",
        isActive && "ring-2 ring-primary",
        isVisited && "opacity-60"
      )}
      shadow="sm"
    >
      <CardBody className="flex flex-row items-center gap-3 p-3">
        {!isVisited && (
          <span className="h-2 w-2 rounded-full bg-primary" />
        )}

        <Chip
          color={
            isHttp
              ? (getMethodColor(request.method) as "success" | "primary" | "warning" | "danger" | "secondary" | "default")
              : "secondary"
          }
          size="sm"
          variant="flat"
          className="min-w-[60px] text-center"
        >
          {isHttp ? request.method : "DNS"}
        </Chip>

        <div className="min-w-0 flex-1">
          <p className="truncate text-sm font-medium">
            {isHttp ? `${request.path}${request.query ?? ""}` : request.domain}
          </p>
          <div className="flex items-center gap-2 text-xs text-default-400">
            <span>{request.ip}</span>
            {request.country && (
              <span className="uppercase">{request.country}</span>
            )}
            <span>{formatRelativeTime(request.date)}</span>
          </div>
        </div>

        {isHttp && request.method !== "GET" && (
          <Chip size="sm" variant="bordered" className="text-xs">
            {request.protocol}
          </Chip>
        )}
      </CardBody>
    </Card>
  );
});
