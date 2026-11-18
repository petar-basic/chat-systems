import { Select } from '@/shared/components/Form/Select';
import type { Variable } from './workflowModel';

export function VariablePicker({
  variables,
  label = 'Insert variable',
  onPick,
  dataQa = 'workflow-step-variable',
}: {
  variables: Variable[];
  label?: string;
  onPick: (token: string) => void;
  dataQa?: string;
}) {
  if (variables.length === 0) return null;
  return (
    <Select
      label={label}
      labelHidden
      compact
      className="w-48"
      value=""
      onChange={(e) => e.target.value && onPick(e.target.value)}
      dataQa={dataQa}
    >
      <option value="">Insert variable…</option>
      {variables.map((variable) => (
        <option key={variable.token} value={variable.token}>
          {variable.label}
        </option>
      ))}
    </Select>
  );
}
