import { useEffect, useState } from "react";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { listen } from "@tauri-apps/api/event";
import { desktop, getSnapshot, sendCommand } from "../lib/api";
import { snapshotSchema, type Snapshot } from "../lib/contracts";

export function useRuntime() {
  const client = useQueryClient();
  const [notice, setNotice] = useState<{ text: string; error: boolean } | null>(
    null,
  );
  const query = useQuery({
    queryKey: ["runtime"],
    queryFn: getSnapshot,
    refetchInterval: 5000,
    retry: false,
    structuralSharing: (previous, next) =>
      previous && (previous as Snapshot).sequence > (next as Snapshot).sequence
        ? previous
        : next,
  });
  const mutation = useMutation({
    mutationFn: sendCommand,
    onSuccess: (snapshot) => {
      client.setQueryData<Snapshot>(["runtime"], (previous) =>
        previous && previous.sequence > snapshot.sequence ? previous : snapshot,
      );
      setNotice({
        text: "Command completed and recorded in audit log.",
        error: false,
      });
    },
    onError: (error) => {
      setNotice({ text: error.message, error: true });
      void client.invalidateQueries({ queryKey: ["runtime"] });
    },
  });
  useEffect(() => {
    if (!query.data) return;
    let disposed = false;
    let socket: WebSocket | undefined;
    let retry: ReturnType<typeof setTimeout> | undefined;
    let unlisten: (() => void) | undefined;
    const receive = (payload: unknown) => {
      const parsed = snapshotSchema.safeParse(payload);
      if (parsed.success)
        client.setQueryData<Snapshot>(["runtime"], (previous) =>
          previous && previous.sequence > parsed.data.sequence
            ? previous
            : parsed.data,
        );
    };
    const connect = () => {
      socket = new WebSocket(
        `${location.protocol === "https:" ? "wss:" : "ws:"}//${location.host}/ws`,
      );
      socket.onmessage = (event) => {
        try {
          const data = JSON.parse(String(event.data));
          if (data.type === "runtime.snapshot") receive(data.payload);
        } catch {
          /* Invalid event is ignored; snapshot polling repairs gaps. */
        }
      };
      socket.onclose = () => {
        if (!disposed) retry = setTimeout(connect, 5000);
      };
    };
    if (desktop) {
      void listen("runtime.snapshot", (event) => receive(event.payload)).then(
        (off) => {
          if (disposed) off();
          else unlisten = off;
        },
      );
    } else connect();
    return () => {
      disposed = true;
      clearTimeout(retry);
      unlisten?.();
      socket?.close();
    };
  }, [client, Boolean(query.data)]);
  return {
    ...query,
    command: mutation.mutateAsync,
    busy:
      mutation.isPending ||
      Boolean(
        query.data?.agents.some((a) =>
          ["Running", "Waiting"].includes(a.status),
        ),
      ),
    cyclePending:
      mutation.isPending && mutation.variables?.action === "run_cycle",
    submittedAt: mutation.submittedAt,
    stoppingCycles:
      mutation.isPending && mutation.variables?.action === "stop_cycles",
    notice,
    clearNotice: () => setNotice(null),
  };
}
