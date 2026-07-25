import type { SVGProps } from "react";

/**
 * Icon geometry adapted from OpenCode's v2 icon component.
 * Source: anomalyco/opencode, packages/ui/src/v2/components/icon.tsx and packages/ui/src/components/icon.tsx
 * License: MIT. See THIRD_PARTY_NOTICES.md.
 */
export type OpenCodeIconName =
  | "arrow-left"
  | "arrow-right"
  | "branch"
  | "check"
  | "chevron-grabber-vertical"
  | "chevron-down"
  | "collapse"
  | "close"
  | "edit"
  | "filetree"
  | "folder"
  | "folder-add-left"
  | "grid-plus"
  | "help"
  | "menu"
  | "magnifying-glass"
  | "plus"
  | "review"
  | "reset"
  | "settings-gear"
  | "split"
  | "status"
  | "trash"
  | "unified"
  | "workspace"
  | "expand";

export interface OpenCodeIconProps extends Omit<SVGProps<SVGSVGElement>, "name"> {
  readonly name: OpenCodeIconName;
  readonly size?: 14 | 16 | 20;
}

const viewBoxes: Record<OpenCodeIconName, string> = {
  "arrow-left": "0 0 20 20",
  "arrow-right": "0 0 20 20",
  branch: "0 0 16 16",
  check: "0 0 16 16",
  "chevron-grabber-vertical": "0 0 20 20",
  "chevron-down": "0 0 16 16",
  collapse: "0 0 16 16",
  close: "0 0 20 20",
  edit: "0 0 16 16",
  filetree: "0 0 16 16",
  folder: "0 0 16 16",
  "folder-add-left": "0 0 16 16",
  "grid-plus": "0 0 16 16",
  help: "0 0 16 16",
  menu: "0 0 16 16",
  "magnifying-glass": "0 0 16 16",
  plus: "0 0 16 16",
  review: "0 0 20 20",
  reset: "0 0 20 20",
  "settings-gear": "0 0 16 16",
  split: "0 0 16 16",
  status: "0 0 20 20",
  trash: "0 0 20 20",
  unified: "0 0 16 16",
  workspace: "0 0 16 16",
  expand: "0 0 16 16",
};

function IconBody({ name }: { readonly name: OpenCodeIconName }) {
  switch (name) {
    case "arrow-left":
      return <path d="M8.33464 4.58398L2.91797 10.0007L8.33464 15.4173M3.33464 10.0007H17.0846" stroke="currentColor" strokeLinecap="square" />;
    case "arrow-right":
      return <path d="M11.6654 4.58398L17.082 10.0007L11.6654 15.4173M16.6654 10.0007H2.91536" stroke="currentColor" strokeLinecap="square" />;
    case "chevron-grabber-vertical":
      return <path d="M6.66675 12.4998L10.0001 15.8332L13.3334 12.4998M6.66675 7.49984L10.0001 4.1665L13.3334 7.49984" stroke="currentColor" strokeLinecap="square" />;
    case "chevron-down":
      return <path d="M5 6.5L8 9.5L11 6.5" stroke="currentColor" />;
    case "check":
      return <path d="M3.53613 8.17857L6.39328 11.75L12.4647 4.25" stroke="currentColor" />;
    case "collapse":
      return <><path d="M8 1V6M11 3L8 6L5 3" stroke="currentColor" /><path d="M8 15V10M11 13L8 10L5 13" stroke="currentColor" /><path d="M4 8H6M7 8H9M10 8H12" stroke="currentColor" /></>;
    case "expand":
      return <><path d="M8.25 6.17773V1.17773M11.25 4.17773L8.25 1.17773L5.25 4.17773" stroke="currentColor" /><path d="M8.25 9.17773V14.1777M11.25 11.1777L8.25 14.1777L5.25 11.1777" stroke="currentColor" /><path d="M4.25 7.67773H12.25" stroke="currentColor" /></>;
    case "magnifying-glass":
      return <path d="M14 14L10.3454 10.3454M6.88889 11.7778C9.58889 11.7778 11.7778 9.58889 11.7778 6.88889C11.7778 4.18889 9.58889 2 6.88889 2C4.18889 2 2 4.18889 2 6.88889C2 9.58889 4.18889 11.7778 6.88889 11.7778Z" stroke="currentColor" />;
    case "split":
      return <><path d="M1 14H15L15 2H1V14Z" stroke="currentColor" /><rect fill="currentColor" fillOpacity="0.5" height="8" width="4" x="3" y="4" /><rect fill="currentColor" fillOpacity="0.5" height="8" width="4" x="9" y="4" /></>;
    case "unified":
      return <><path d="M3.00001 4.00045L12.9998 4L13 6.99955L3 7L3.00001 4.00045Z" fill="currentColor" fillOpacity="0.5" /><path d="M3.0001 9H13L12.9999 12H3L3.0001 9Z" fill="currentColor" fillOpacity="0.5" /><path d="M1 14H15L15 2H1V14Z" stroke="currentColor" /></>;
    case "edit":
      return <path d="M13.5555 8.21534V13.5556H2.44434V2.4445H7.78462M6.88878 9.11119S8.96327 9.0367 9.69678 8.3032L14.0301 3.96986C14.5824 3.4176 14.5824 2.52213 14.0301 1.96986C13.4778 1.4176 12.5824 1.4176 12.0301 1.96986L7.69678 6.3032C7.00513 6.99484 6.88878 9.11119 6.88878 9.11119Z" stroke="currentColor" />;
    case "folder-add-left":
      return <path d="M7.5 13.3333H1.5V2H6.83333L8.83333 4H14.8333V6M10.1667 11.3333H15.5M12.8333 8.66667V14" stroke="currentColor" strokeLinecap="square" strokeMiterlimit="10" />;
    case "folder":
      return <path d="M2.545 3.364V12.636H13.455V5H8.545L6.909 3.364H2.545Z" stroke="currentColor" strokeLinecap="square" strokeMiterlimit="10" />;
    case "branch":
      return <path d="M5.118 5.686V10.314M5.118 5.686C5.97 5.686 6.661 4.995 6.661 4.143C6.661 3.291 5.97 2.6 5.118 2.6C4.266 2.6 3.575 3.291 3.575 4.143C3.575 4.995 4.266 5.686 5.118 5.686ZM5.118 10.314C4.266 10.314 3.575 11.005 3.575 11.857C3.575 12.709 4.266 13.4 5.118 13.4C5.97 13.4 6.661 12.709 6.661 11.857M5.118 10.314C5.97 10.314 6.661 11.005 6.661 11.857M10.882 5.686C11.734 5.686 12.425 4.995 12.425 4.143C12.425 3.291 11.734 2.6 10.882 2.6C10.03 2.6 9.339 3.291 9.339 4.143C9.339 4.995 10.03 5.686 10.882 5.686ZM10.882 5.686V9.457C10.882 10.783 9.807 11.857 8.482 11.857H6.661" stroke="currentColor" strokeLinecap="round" strokeLinejoin="round" />;
    case "grid-plus":
      return <path d="M13.9948 11.668H9.32812M11.6641 9.33203V13.9987M6.66667 9.33203V13.9987H2V9.33203H6.66667ZM6.66667 2V6.66667H2V2H6.66667ZM13.9948 2V6.66667H9.32812V2H13.9948Z" stroke="currentColor" strokeLinecap="square" strokeMiterlimit="10" />;
    case "help":
      return <path d="M6.33345 6.33349V5.00015H9.66679V7.00015L8.00015 8.00015V9.66679M8.27485 11.6819H7.71897M14.4446 8.00011C14.4446 11.5593 11.5593 14.4446 8.00011 14.4446C4.44094 14.4446 1.55566 11.5593 1.55566 8.00011C1.55566 4.44094 4.44094 1.55566 8.00011 1.55566C11.5593 1.55566 14.4446 4.44094 14.4446 8.00011Z" stroke="currentColor" strokeLinecap="square" />;
    case "status":
      return <path d="M2 10V18H18V10M2 10V2H18V10M2 10H18M5 6H9M5 14H9" stroke="currentColor" />;
    case "reset":
      return <path d="M5.83333 4.16406L2.5 7.4974L5.83333 10.8307M3.33333 7.4974H17.9167V15.4141H10" stroke="currentColor" strokeLinecap="square" />;
    case "trash":
      return <path d="M4.58342 17.9134L4.58369 17.4134L4.22787 17.5384L4.22766 18.0384H4.58342V17.9134ZM15.4167 17.9134V18.0384H15.7725L15.7723 17.5384L15.4167 17.9134ZM2.08342 3.95508V3.45508H1.58342V3.95508H2.08342V4.45508V3.95508ZM17.9167 4.45508V4.95508H18.4167V4.45508H17.9167V3.95508V4.45508ZM4.16677 4.58008L3.66701 4.5996L4.22816 17.5379L4.72792 17.4934L5.22767 17.4489L4.66652 4.54055L4.16677 4.58008ZM4.58342 18.0384V17.9134H15.4167V18.0384V18.5384H4.58342V18.0384ZM15.4167 17.9134L15.8332 17.5379L16.2498 4.5996L15.7501 4.58008L15.2503 4.56055L14.8337 17.4989L15.4167 17.9134ZM15.8334 4.58008V4.08008H4.16677V4.58008V5.08008H15.8334V4.58008ZM2.08342 4.45508V4.95508H4.16677V4.58008V4.08008H2.08342V4.45508ZM15.8334 4.58008V5.08008H17.9167V4.45508V3.95508H15.8334V4.58008ZM6.83951 4.35149L7.432 4.55047C7.79251 3.47701 8.80699 2.70508 10.0001 2.70508V2.20508V1.70508C8.25392 1.70508 6.77335 2.83539 6.24702 4.15251L6.83951 4.35149ZM10.0001 2.20508V2.70508C11.1932 2.70508 12.2077 3.47701 12.5682 4.55047L13.1607 4.35149L13.7532 4.15251C13.2269 2.83539 11.7463 1.70508 10.0001 1.70508V2.20508Z" fill="currentColor" />;
    case "menu":
      return <path d="M2 8H14M2 4.664H14M2 11.336H14" stroke="currentColor" />;
    case "plus":
      return <><path d="M8 2.88867V13.1109" stroke="currentColor" strokeLinejoin="round" /><path d="M2.88867 8H13.1109" stroke="currentColor" strokeLinejoin="round" /></>;
    case "settings-gear":
      return <><path d="M7.99998 1.3335L14 4.66683V11.3335L7.99998 14.6668L2 11.3335V4.66683L7.99998 1.3335Z" stroke="currentColor" /><path d="M9.99998 8.00016C9.99998 9.10476 9.10458 10.0002 7.99998 10.0002C6.89538 10.0002 5.99998 9.10476 5.99998 8.00016C5.99998 6.89556 6.89538 6.00016 7.99998 6.00016C9.10458 6.00016 9.99998 6.89556 9.99998 8.00016Z" stroke="currentColor" /></>;
    case "close":
      return <path d="M14.4446 5.55566L5.55566 14.4446M5.55566 5.55566L14.4446 14.4446" stroke="currentColor" strokeLinejoin="round" />;
    case "filetree":
      return <><path d="M2.5 1.5V12.2484H6.75M2.5 4.74838H6.75" stroke="currentColor" /><rect fill="none" height="3" stroke="currentColor" width="6" x="8.5" y="3.2168" /><rect fill="none" height="3" stroke="currentColor" width="6" x="8.5" y="10.75" /></>;
    case "review":
      return <path d="M7 14.5H13M7 7.99512H10.0049M10.0049 7.99512H13M10.0049 7.99512V5M10.0049 7.99512V11M18 18V2L2 2V18H18Z" stroke="currentColor" />;
    case "workspace":
      return <><path d="M2 10.668V14.0013H10.6667M13.9974 10.6667V2H2.66406M13.9974 10.668V14.0013H10.6641M2 10V2H5.33333" stroke="currentColor" strokeLinecap="square" strokeMiterlimit="10" /><path d="M10.6693 10.6654V5.33203H5.33594V10.6654H10.6693Z" fill="currentColor" /></>;
  }
}

export function OpenCodeIcon({ name, size = 16, ...props }: OpenCodeIconProps) {
  return (
    <svg
      {...props}
      aria-hidden={props["aria-hidden"] ?? true}
      data-forge-icon={name}
      fill="none"
      height={size}
      viewBox={viewBoxes[name]}
      width={size}
      xmlns="http://www.w3.org/2000/svg"
    >
      <IconBody name={name} />
    </svg>
  );
}
