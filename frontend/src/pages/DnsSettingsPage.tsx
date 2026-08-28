import { useState, useCallback, useEffect, useRef } from "react";
import {
  Card,
  Button,
  Input,
  Select,
  ListBox,
  Chip,
} from "@heroui/react";
import { Plus, Trash2, Network } from "lucide-react";
import { toast } from "sonner";
import { useSessionStore } from "@/stores/sessionStore";
import { useQuery, useMutation, useQueryClient } from "@tanstack/react-query";
import { apiClient } from "@/api/client";
import type { DnsRecord, DnsRecordType } from "@/types";
import { getDnsTypeColor } from "@/lib/utils";
import { getDnsDomain } from "@/lib/config";

const DNS_TYPES: DnsRecordType[] = ["A", "AAAA", "CNAME", "TXT"];

export function DnsSettingsPage() {
  const sessions = useSessionStore((s) => s.sessions);
  const activeSubdomain = useSessionStore((s) => s.activeSubdomain);
  const session = sessions.find((s) => s.subdomain === activeSubdomain);
  const queryClient = useQueryClient();

  const [newRecord, setNewRecord] = useState<Partial<DnsRecord>>({
    domain: "",
    type: "A",
    value: "",
  });

  const { data: records = [], isLoading } = useQuery({
    queryKey: ["dns", session?.token],
    queryFn: () => apiClient.getDnsRecords(session!.token),
    enabled: Boolean(session?.token),
  });

  const updateDnsMutation = useMutation({
    mutationFn: (newRecords: DnsRecord[]) =>
      apiClient.updateDnsRecords(session!.token, newRecords),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ["dns", session?.token] });
      toast.success("DNS records saved");
    },
    onError: () => {
      toast.error("Failed to save DNS records");
    },
  });

  const handleAddRecord = useCallback(() => {
    if (!newRecord.domain || !newRecord.value || !newRecord.type) {
      toast.error("Please fill in all fields");
      return;
    }

    const record: DnsRecord = {
      domain: newRecord.domain,
      type: newRecord.type as DnsRecordType,
      value: newRecord.value,
    };

    updateDnsMutation.mutate([...records, record]);
    setNewRecord({ domain: "", type: "A", value: "" });
  }, [newRecord, records, updateDnsMutation]);

  const handleDeleteRecord = useCallback(
    (index: number) => {
      const newRecords = records.filter((_, i) => i !== index);
      updateDnsMutation.mutate(newRecords);
    },
    [records, updateDnsMutation],
  );

  // Ref to always have latest handleAddRecord
  const handleAddRef = useRef(handleAddRecord);
  useEffect(() => {
    handleAddRef.current = handleAddRecord;
  }, [handleAddRecord]);

  // Global Ctrl/Cmd+S to add record
  useEffect(() => {
    const handleKeyDown = (e: KeyboardEvent) => {
      if ((e.metaKey || e.ctrlKey) && e.key === "s") {
        e.preventDefault();
        e.stopPropagation();
        handleAddRef.current();
      }
    };
    window.addEventListener("keydown", handleKeyDown, true);
    return () => window.removeEventListener("keydown", handleKeyDown, true);
  }, []);

  if (!session) {
    return <div>No session selected</div>;
  }

  return (
    <div className="flex flex-col gap-4">
      {/* Info */}
      <Card>
        <Card.Content className="gap-2 flex flex-col">
          <div className="flex items-center gap-2">
            <Network className="h-5 w-5 text-primary" />
            <h2 className="text-lg font-semibold">DNS Settings</h2>
          </div>
          <p className="text-sm text-default-500">
            Configure custom DNS responses for your subdomain. Requests to{" "}
            <code className="text-primary">
              *.{session.subdomain}.{getDnsDomain()}
            </code>{" "}
            will return these records.
          </p>
        </Card.Content>
      </Card>

      {/* Add new record */}
      <Card>
        <Card.Header>
          <h3 className="text-base font-semibold">Add New Record</h3>
        </Card.Header>
        <div className="border-b border-default-200" />
        <Card.Content className="gap-4 flex flex-col">
          {/* Domain - full width on mobile */}
          <div>
            <label className="text-sm text-default-500 block mb-1">Domain</label>
            <Input
              placeholder="subdomain"
              value={newRecord.domain}
              onChange={(e) => setNewRecord((r) => ({ ...r, domain: e.target.value }))}
            />
          </div>

          {/* Type + Value row */}
          <div className="flex flex-col gap-4 md:flex-row">
            <div className="w-full md:w-32">
              <label className="text-sm text-default-500 block mb-1">Type</label>
              <Select
                selectedKey={newRecord.type || "A"}
                onSelectionChange={(key) => {
                  if (key)
                    setNewRecord((r) => ({
                      ...r,
                      type: String(key) as DnsRecordType,
                    }));
                }}
              >
                <Select.Trigger className="w-full" />
                <Select.Popover>
                  <ListBox>
                    {DNS_TYPES.map((type) => (
                      <ListBox.Item key={type}>{type}</ListBox.Item>
                    ))}
                  </ListBox>
                </Select.Popover>
              </Select>
            </div>
            <div className="flex-1">
              <label className="text-sm text-default-500 block mb-1">Value</label>
              <Input
                placeholder={
                  newRecord.type === "A"
                    ? "1.2.3.4"
                    : newRecord.type === "AAAA"
                      ? "2001:db8::1"
                      : newRecord.type === "CNAME"
                        ? "example.com"
                        : "txt value"
                }
                value={newRecord.value}
                onChange={(e) => setNewRecord((r) => ({ ...r, value: e.target.value }))}
              />
            </div>
          </div>

          {/* Add button - full width on mobile */}
          <Button
            variant="primary"
            onPress={handleAddRecord}
            className="w-full md:w-auto md:self-start"
          >
            <Plus className="h-4 w-4" />
            Add Record
          </Button>

          <p className="text-xs text-default-400">
            Will respond to:{" "}
            <code className="break-all text-primary">
              {newRecord.domain || "*"}.{session.subdomain}.{getDnsDomain()}
            </code>
            {newRecord.type === "A" &&
              " • Use % to separate multiple IPs for random selection"}
          </p>
        </Card.Content>
      </Card>

      {/* Existing records */}
      <Card>
        <Card.Header>
          <h3 className="text-base font-semibold">Current Records</h3>
        </Card.Header>
        <div className="border-b border-default-200" />
        <Card.Content className="gap-2 flex flex-col">
          {isLoading ? (
            <p className="text-default-400">Loading...</p>
          ) : records.length === 0 ? (
            <p className="text-default-400">No custom DNS records configured</p>
          ) : (
            records.map((record, index) => (
              <div
                key={index}
                className="flex flex-col gap-2 rounded-lg bg-default-100 p-3 md:flex-row md:items-center md:gap-4"
              >
                {/* Type badge + domain */}
                <div className="flex items-center gap-2 md:flex-1">
                  <Chip
                    color={getDnsTypeColor(record.type) as "success" | "warning" | "danger" | "default" | "accent"}
                    variant="soft"
                    size="sm"
                    className="min-w-[50px]"
                  >
                    {record.type}
                  </Chip>
                  <span className="truncate font-mono text-sm">
                    {record.domain}.{session.subdomain}.{getDnsDomain()}
                  </span>
                </div>

                {/* Value + delete */}
                <div className="flex items-center gap-2 pl-[58px] md:flex-1 md:pl-0">
                  <span className="hidden font-mono text-sm text-default-500 md:inline">
                    →
                  </span>
                  <span className="flex-1 truncate font-mono text-sm">
                    {record.value}
                  </span>
                  <Button
                    isIconOnly
                    size="sm"
                    variant="danger"
                    onPress={() => handleDeleteRecord(index)}
                  >
                    <Trash2 className="h-4 w-4" />
                  </Button>
                </div>
              </div>
            ))
          )}
        </Card.Content>
      </Card>
    </div>
  );
}
