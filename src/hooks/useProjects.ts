import { useCallback, useEffect } from "react";
import { toast } from "sonner";
import { errorMessage } from "@/lib/utils";
import { useAppStore } from "@/stores/appStore";
import * as api from "@/lib/tauri";

export function useProjects() {
  const { projects, setProjects, selectedProjectIds, toggleProjectSelection, clearProjectSelection } =
    useAppStore();

  const fetchProjects = useCallback(async () => {
    try {
      const data = await api.listProjects();
      setProjects(data);
    } catch (err) {
      toast.error("Failed to fetch projects", { description: errorMessage(err) });
    }
  }, [setProjects]);

  const addProjectByUrl = useCallback(
    async (url: string) => {
      const project = await api.addProjectByUrl(url);
      setProjects([...projects, project]);
      return project;
    },
    [projects, setProjects]
  );

  const removeProject = useCallback(
    async (id: string) => {
      await api.removeProject(id);
      setProjects(projects.filter((p) => p.id !== id));
      if (selectedProjectIds.includes(id)) {
        toggleProjectSelection(id);
      }
    },
    [projects, setProjects, selectedProjectIds, toggleProjectSelection]
  );

  useEffect(() => {
    fetchProjects();
  }, [fetchProjects]);

  return {
    projects,
    selectedProjectIds,
    toggleProjectSelection,
    clearProjectSelection,
    fetchProjects,
    addProjectByUrl,
    removeProject,
  };
}
