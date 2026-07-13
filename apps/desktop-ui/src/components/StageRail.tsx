import { Check, Circle } from "lucide-react";

const stages = ["Context", "Plan", "Review", "Apply", "Verify"] as const;

export type TaskStage = (typeof stages)[number];

export function StageRail({ current }: { current: TaskStage }) {
  const currentIndex = stages.indexOf(current);

  return (
    <ol aria-label="Task progress" className="stage-rail">
      {stages.map((stage, index) => {
        const isComplete = index < currentIndex;
        const isCurrent = index === currentIndex;

        return (
          <li
            aria-current={isCurrent ? "step" : undefined}
            className={isComplete ? "is-complete" : isCurrent ? "is-current" : ""}
            key={stage}
          >
            <span className="stage-rail__line" />
            <span className="stage-rail__marker">
              {isComplete ? <Check aria-hidden="true" size={14} strokeWidth={2.4} /> : <Circle aria-hidden="true" size={10} />}
            </span>
            <span>{stage}</span>
          </li>
        );
      })}
    </ol>
  );
}
