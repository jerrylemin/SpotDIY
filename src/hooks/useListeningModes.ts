import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";

import { enterTemporaryMode, exitTemporaryMode, getListeningModeState, setPrivateSession } from "../services/ipc";

export const LISTENING_MODE_QUERY_KEY = ["listening-mode"] as const;

export function useListeningModes() {
  const queryClient = useQueryClient();
  const state = useQuery({
    queryKey: LISTENING_MODE_QUERY_KEY,
    queryFn: getListeningModeState,
    retry: 1,
  });
  const onSuccess = () => queryClient.invalidateQueries({ queryKey: LISTENING_MODE_QUERY_KEY });
  const privateSession = useMutation({ mutationFn: setPrivateSession, onSuccess });
  const temporaryEnter = useMutation({ mutationFn: enterTemporaryMode, onSuccess });
  const temporaryExit = useMutation({ mutationFn: exitTemporaryMode, onSuccess });
  return { state, privateSession, temporaryEnter, temporaryExit };
}
