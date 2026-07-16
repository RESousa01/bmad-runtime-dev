import { Button } from "@sapphirus/ui";
import {
  Activity,
  ChevronDown,
  FileCode2,
  FolderKanban,
  GitCompareArrows,
  MessageSquare,
  Search,
  Settings,
  UserRound,
  type LucideIcon,
} from "lucide-react";
import type { PrimaryView } from "../data/demo";
import { BrandMark } from "./BrandMark";

interface NavigationItem {
  icon: LucideIcon;
  id: PrimaryView;
  label: string;
}

const primaryItems: NavigationItem[] = [
  { id: "agent", label: "Agent", icon: MessageSquare },
  { id: "workspaces", label: "Workspaces", icon: FolderKanban },
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
      <div className="sidebar-identity">
        <div className="sidebar-product" title="Sapphirus">
          <BrandMark size={22} />
          <strong>Sapphirus</strong>
          <ChevronDown aria-hidden="true" size={14} strokeWidth={1.8} />
        </div>
        <Button
          aria-label="Search sessions"
          className="sidebar-search"
          isDisabled
          size="icon"
          title="Search sessions"
          variant="quiet"
        >
          <Search aria-hidden="true" size={16} strokeWidth={1.8} />
        </Button>
      </div>
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
            <span aria-hidden="true" className="global-nav-item__indicator" />
            <Icon
              aria-hidden="true"
              className="global-nav-item__icon"
              size={21}
              strokeWidth={1.7}
            />
            <span className="global-nav-item__label">{label}</span>
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
          <span aria-hidden="true" className="global-nav-item__indicator" />
          <Settings
            aria-hidden="true"
            className="global-nav-item__icon"
            size={21}
            strokeWidth={1.7}
          />
          <span className="global-nav-item__label">Settings</span>
        </Button>
        <Button
          aria-label="Account"
          className="global-nav-item global-nav-item--account"
          onPress={onAccount}
          title="Account"
          variant="quiet"
        >
          <span aria-hidden="true" className="global-nav-item__indicator" />
          <span aria-hidden="true" className="sidebar-avatar">RA</span>
          <span className="global-nav-item__label">Local account</span>
          <UserRound aria-hidden="true" className="global-nav-item__account-icon" size={15} />
        </Button>
      </div>
    </nav>
  );
}
