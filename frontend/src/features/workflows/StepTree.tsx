import { Plus } from 'lucide-react';
import { Button } from '@/shared/components/Button/Button';
import type { Channel, WorkspaceMember } from '@/stores/workspace';
import { StepCard } from './StepCard';
import {
  addStep,
  blankStep,
  inLane,
  moveStep,
  removeStep,
  replaceStep,
  type Definition,
  type FlatStep,
  type Lane,
  type Step,
  type StepProvider,
} from './workflowModel';

export interface TreeProps {
  flat: FlatStep[];
  definition: Definition;
  workspaceId: string;
  instanceUrl?: string;
  channels: Channel[];
  people: WorkspaceMember[];
  members: WorkspaceMember[];
  providers?: StepProvider[];
  full: boolean;
  readOnly: boolean;
  onEdit: (change: (steps: Step[]) => Step[]) => void;
}

export function StepTree({ lane, ...props }: TreeProps & { lane: Lane | null }) {
  const nodes = inLane(props.flat, lane);
  return (
    <ol className="space-y-3" aria-label={lane ? 'Steps on this path' : 'Steps'}>
      {nodes.map((node) => (
        <li key={node.position}>
          <StepCard
            node={node}
            definition={props.definition}
            workspaceId={props.workspaceId}
            instanceUrl={props.instanceUrl}
            channels={props.channels}
            people={props.people}
            members={props.members}
            providers={props.providers}
            removable={lane !== null || node.siblings > 1}
            readOnly={props.readOnly}
            onChange={(step) => props.onEdit((steps) => replaceStep(steps, node.position, step))}
            onMove={(offset) => props.onEdit((steps) => moveStep(steps, node.position, offset))}
            onRemove={() => props.onEdit((steps) => removeStep(steps, node.position))}
          />
          {node.step.type === 'branch' && (
            <div className="mt-3 space-y-3 pl-4 sm:pl-8">
              <BranchLane {...props} lane={[node.position, true]} />
              <BranchLane {...props} lane={[node.position, false]} />
            </div>
          )}
        </li>
      ))}
    </ol>
  );
}

function BranchLane({ lane, ...props }: TreeProps & { lane: Lane }) {
  const [branch, matched] = lane;
  const title = matched ? 'If it matches' : 'Otherwise';
  const empty = inLane(props.flat, lane).length === 0;
  return (
    <section
      className="space-y-3 border-l-2 border-accent/40 pl-4"
      aria-label={`Step ${branch + 1}: ${title.toLowerCase()}`}
      data-qa="workflow-lane"
      data-lane={matched ? 'then' : 'otherwise'}
    >
      <h4 className="text-xs font-semibold uppercase tracking-wide text-muted">{title}</h4>
      {empty ? (
        <p className="text-xs text-muted">No steps: the run goes straight on.</p>
      ) : (
        <StepTree {...props} lane={lane} />
      )}
      {!props.readOnly && !props.full && (
        <Button
          size="sm"
          variant="ghost"
          icon={<Plus className="h-3.5 w-3.5" />}
          onClick={() =>
            props.onEdit((steps) =>
              addStep(steps, lane, blankStep('send_message', props.channels[0]?.id ?? '')),
            )
          }
          data-qa="workflow-lane-add"
        >
          Add a step
        </Button>
      )}
    </section>
  );
}
