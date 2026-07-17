import { Button } from "@sapphirus/ui";
import {
  ChevronDown,
  Settings,
  SquarePen,
  UserRound,
} from "lucide-react";
import { BrandMark } from "../BrandMark";

export interface AppSidebarTask {
  id: string;
  title: string;
  unread?: boolean;
  updatedAt?: string;
}

export interface AppSidebarProps {
  canCreateTask: boolean;
  onNewTask: () => void;
  onOpenAccount: () => void;
  onOpenSettings: () => void;
  onOpenWorkspaceManager: () => void;
  onSelectTask: (taskId: string) => void;
  selectedTaskId: string | null;
  tasks: readonly AppSidebarTask[];
  workspaceLabel: string;
  workspaceStatus: string;
}

export function AppSidebar({
  canCreateTask,
  onNewTask,
  onOpenAccount,
  onOpenSettings,
  onOpenWorkspaceManager,
  onSelectTask,
  selectedTaskId,
  tasks,
  workspaceLabel,
  workspaceStatus,
}: AppSidebarProps) {
  return (
    <nav aria-label="Sidebar" className="app-sidebar">
      <div className="app-sidebar__brand" title="Sapphirus">
        <BrandMark size={22} />
        <strong>Sapphirus</strong>
      </div>

      <Button
        aria-label={`Manage workspace ${workspaceLabel}`}
        className="app-sidebar__workspace"
        onPress={onOpenWorkspaceManager}
        variant="quiet"
      >
        <span className="app-sidebar__workspace-copy">
          <span className="app-sidebar__eyebrow">Workspace</span>
          <strong>{workspaceLabel}</strong>
          <span className="app-sidebar__workspace-status" title={workspaceStatus}>{workspaceStatus}</span>
        </span>
        <ChevronDown aria-hidden="true" size={16} strokeWidth={1.8} />
      </Button>

      <Button
        className="app-sidebar__new-task"
        isDisabled={!canCreateTask}
        onPress={onNewTask}
        size="large"
        variant="primary"
      >
        <SquarePen aria-hidden="true" size={17} strokeWidth={1.8} />
        New task
      </Button>

      <section aria-label="Task history" className="app-sidebar__tasks">
        <h2>Tasks</h2>
        <div className="app-sidebar__task-list" role="list">
          {tasks.length === 0 ? (
            <p className="app-sidebar__empty">No tasks yet</p>
          ) : tasks.map((task) => (
            <div key={task.id} role="listitem">
              <Button
                {...(selectedTaskId === task.id
                  ? { "aria-current": "page" as const }
                  : {})}
                className="app-sidebar__task"
                onPress={() => onSelectTask(task.id)}
                variant="quiet"
              >
                <span className="app-sidebar__task-title">{task.title}</span>
                {task.updatedAt || task.unread ? (
                  <span className="app-sidebar__task-meta">
                    {task.updatedAt ? <span>{task.updatedAt}</span> : null}
                    {task.unread ? (
                      <>
                        <span aria-hidden="true" className="app-sidebar__unread" />
                        <span className="sr-only">Unread</span>
                      </>
                    ) : null}
                  </span>
                ) : null}
              </Button>
            </div>
          ))}
        </div>
      </section>

      <div className="app-sidebar__utilities">
        <Button
          aria-label="Settings"
          className="app-sidebar__utility"
          onPress={onOpenSettings}
          variant="quiet"
        >
          <Settings aria-hidden="true" size={18} strokeWidth={1.8} />
          <span>Settings</span>
        </Button>
        <Button
          aria-label="Account"
          className="app-sidebar__utility"
          onPress={onOpenAccount}
          variant="quiet"
        >
          <span aria-hidden="true" className="app-sidebar__avatar">
            <UserRound size={14} strokeWidth={1.8} />
          </span>
          <span>Account</span>
        </Button>
      </div>
    </nav>
  );
}
