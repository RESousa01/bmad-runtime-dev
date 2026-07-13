import type { ReactNode } from "react";
import {
  Tab as AriaTab,
  TabList as AriaTabList,
  TabPanel as AriaTabPanel,
  Tabs as AriaTabs,
  type TabListProps as AriaTabListProps,
  type TabPanelProps as AriaTabPanelProps,
  type TabProps as AriaTabProps,
  type TabsProps as AriaTabsProps,
} from "react-aria-components";

export function Tabs(props: AriaTabsProps) {
  return <AriaTabs {...props} />;
}

export interface TabListProps<T extends object> extends AriaTabListProps<T> {
  className?: string;
}

export function TabList<T extends object>({ className = "", ...props }: TabListProps<T>) {
  return <AriaTabList className={`sapphirus-tab-list ${className}`} {...props} />;
}

export interface TabProps extends Omit<AriaTabProps, "children"> {
  children: ReactNode;
  className?: string;
}

export function Tab({ children, className = "", ...props }: TabProps) {
  return (
    <AriaTab className={`sapphirus-tab ${className}`} {...props}>
      {children}
    </AriaTab>
  );
}

export interface TabPanelProps extends AriaTabPanelProps {
  className?: string;
}

export function TabPanel({ className = "", ...props }: TabPanelProps) {
  return <AriaTabPanel className={`sapphirus-tab-panel ${className}`} {...props} />;
}
