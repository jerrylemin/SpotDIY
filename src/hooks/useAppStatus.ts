import { useQuery } from "@tanstack/react-query";

import { getAppStatus } from "../services/ipc";

export function useAppStatus() {
  return useQuery({
    queryKey: ["app-status"],
    queryFn: getAppStatus,
    staleTime: Number.POSITIVE_INFINITY,
    retry: 1,
  });
}
