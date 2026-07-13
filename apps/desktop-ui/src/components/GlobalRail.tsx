import { Button } from "@sapphirus/ui";
import {
  Activity,
  Bot,
  FileCode2,
  FolderKanban,
  GitCompareArrows,
  Settings,
  UserRound,
  type LucideIcon,
} from "lucide-react";
import type { PrimaryView } from "../data/demo";

interface NavigationItem {
  icon: LucideIcon;
  id: PrimaryView;
  label: string;
}

const primaryItems: NavigationItem[] = [
  { id: "workspaces", label: "Workspaces", icon: FolderKanban },
  { id: "agent", label: "Agent", icon: Bot },
  { id: "explorer", label: "Explorer", icon: FileCode2 },
  { id: "changes", label: "Changes", icon: GitCompareArrows },
  { id: "activity", label: "Activity", icon: Activity },
];

export interface GlobalRailProps {
  activeView: PrimaryView;
  isInert?: boolean;
  onAccount: () => void;
  onNavigate: (view: PrimaryView) => void;
  onSettings: () => void;
}

export function GlobalRail({ activeView, isInert = false, onAccount, onNavigate, onSettings }: GlobalRailProps) {
  return (
    <nav aria-label="Primary" className="global-rail" inert={isInert}>
      <div className="global-rail__primary">
        {primaryItems.map(({ icon: Icon, id, label }) => (
          <Button
            aria-label={label}
            {...(activeView === id ? { "aria-current": "page" as const } : {})}
            className="global-nav-item"
            key={id}
            onPress={() => onNavigate(id)}
            title={label}
            variant="quiet"
          >
            <Icon aria-hidden="true" size={23} strokeWidth={1.65} />
            <span>{label}</span>
          </Button>
        ))}
      </div>
      <div className="global-rail__utility">
        <Button
          aria-label="Settings"
          className="global-nav-item"
          onPress={onSettings}
          title="Settings"
          variant="quiet"
        >
          <Settings aria-hidden="true" size={23} strokeWidth={1.65} />
          <span>Settings</span>
        </Button>
        <Button
          aria-label="Account"
          className="global-nav-item"
          onPress={onAccount}
          title="Account"
          variant="quiet"
        >
          <UserRound aria-hidden="true" size={23} strokeWidth={1.65} />
          <span>Account</span>
        </Button>
      </div>
    </nav>
  );
}
