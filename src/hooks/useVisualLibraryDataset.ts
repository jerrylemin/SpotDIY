import { useQuery } from "@tanstack/react-query";

import { getVisualLibraryDataset } from "../services/ipc";
import type { VisualDatasetRequest } from "../types/domain";

export const VISUAL_LIBRARY_DATASET_QUERY_KEY = ["visual-library-dataset"] as const;

export function useVisualLibraryDataset(request: VisualDatasetRequest) {
  return useQuery({
    queryKey: [...VISUAL_LIBRARY_DATASET_QUERY_KEY, request],
    queryFn: () => getVisualLibraryDataset(request),
    staleTime: 5_000,
    retry: 1,
  });
}
