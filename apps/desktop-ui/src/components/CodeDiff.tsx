import { MoreHorizontal } from "lucide-react";
import { diffLines } from "../data/demo";

export function CodeDiff() {
  return (
    <section className="diff-card" aria-labelledby="diff-file-name">
      <header>
        <span aria-hidden="true" className="file-kind-icon">TS</span>
        <strong id="diff-file-name">src/scan/workspace_scanner.ts</strong>
        <MoreHorizontal aria-hidden="true" size={17} />
      </header>
      <div className="diff-scroll" tabIndex={0}>
        <table>
          <caption className="sr-only">Proposed code changes</caption>
          <thead>
            <tr>
              <th scope="col">
                <span className="sr-only">Change type</span>
              </th>
              <th scope="col">
                <span className="sr-only">Old line</span>
              </th>
              <th scope="col">
                <span className="sr-only">New line</span>
              </th>
              <th scope="col">
                <span className="sr-only">Code</span>
              </th>
            </tr>
          </thead>
          <tbody>
            {diffLines.map((line, index) => (
              <tr className={`diff-line diff-line--${line.kind}`} key={`${line.kind}-${index}`}>
                <td className="diff-line__sign">
                  <span aria-hidden="true">
                    {line.kind === "added" ? "+" : line.kind === "removed" ? "−" : ""}
                  </span>
                  <span className="sr-only">
                    {line.kind === "added"
                      ? "Added line"
                      : line.kind === "removed"
                        ? "Removed line"
                        : "Context line"}
                  </span>
                </td>
                <td aria-label={line.oldNumber ? `Old line ${line.oldNumber}` : ""} className="diff-line__number">
                  {line.oldNumber ?? ""}
                </td>
                <td aria-label={line.newNumber ? `New line ${line.newNumber}` : ""} className="diff-line__number">
                  {line.newNumber ?? ""}
                </td>
                <td className="diff-line__code">
                  <code>{line.text || "\u00a0"}</code>
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </section>
  );
}
