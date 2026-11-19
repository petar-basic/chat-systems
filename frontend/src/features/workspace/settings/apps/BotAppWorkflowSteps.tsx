import { useState, type FormEvent } from 'react';
import { useTranslation } from 'react-i18next';
import { Plus, Trash2 } from 'lucide-react';
import type { components } from '@/api/schema';
import { useBotAppWorkflowSteps, useSaveBotAppWorkflowSteps, type WorkflowStepDraft } from '@/features/apps';
import type { BotApp } from '@/features/workspace/hooks/useBotApps';
import { Button } from '@/shared/components/Button/Button';
import { Checkbox } from '@/shared/components/Form/Checkbox';
import { IconButton } from '@/shared/components/IconButton/IconButton';
import { Select } from '@/shared/components/Form/Select';
import { TextInput } from '@/shared/components/Form/TextInput';
import { toUserMessage } from '@/lib/errors';

type Param = components['schemas']['StepParam'];
type ParamKind = components['schemas']['StepParamKind'];

const MAX_STEPS = 20;
const MAX_PARAMS = 10;
const KINDS: ParamKind[] = ['text', 'number', 'boolean', 'person', 'channel', 'date'];

interface Props {
  app: BotApp;
}

export function BotAppWorkflowSteps({ app }: Props) {
  const listed = useBotAppWorkflowSteps(app.id);
  if (!listed.data) return null;
  const ready = app.scopes.includes('workflow.steps:execute') && app.events.includes('function_executed');
  return (
    <StepsForm
      key={listed.dataUpdatedAt}
      appId={app.id}
      ready={ready}
      initial={listed.data.map(({ callback_id, title, description, inputs, outputs }) => ({
        callback_id,
        title,
        description,
        inputs,
        outputs,
      }))}
    />
  );
}

interface FormProps {
  appId: string;
  ready: boolean;
  initial: WorkflowStepDraft[];
}

function StepsForm({ appId, ready, initial }: FormProps) {
  const { t } = useTranslation('workspace');
  const save = useSaveBotAppWorkflowSteps(appId);
  const [drafts, setDrafts] = useState<WorkflowStepDraft[]>(initial);

  const change = (index: number, patch: Partial<WorkflowStepDraft>) =>
    setDrafts((current) => current.map((draft, i) => (i === index ? { ...draft, ...patch } : draft)));

  const add = () =>
    setDrafts((current) => [
      ...current,
      { callback_id: '', title: '', description: '', inputs: [], outputs: [] },
    ]);

  const submit = (e: FormEvent) => {
    e.preventDefault();
    save.mutate(drafts);
  };

  return (
    <form
      onSubmit={submit}
      className="space-y-3 border-t border-line/50 pt-3"
      data-qa="bot-app-workflow-steps"
    >
      <div>
        <h4 className="text-xs font-medium text-fg-dim">{t('settings.apps.workflowSteps.title')}</h4>
        <p className="text-xs text-muted">{t('settings.apps.workflowSteps.intro', { max: MAX_STEPS })}</p>
        {!ready && drafts.length > 0 && (
          <p className="mt-1 text-xs text-warning" data-qa="bot-app-workflow-steps-needs">
            {t('settings.apps.workflowSteps.needs')}
          </p>
        )}
      </div>
      {drafts.map((draft, index) => (
        <div key={index} className="space-y-3 rounded-lg bg-raised/40 p-3" data-qa="bot-app-workflow-step">
          <div className="grid grid-cols-1 gap-2 sm:grid-cols-2">
            <TextInput
              label={t('settings.apps.workflowSteps.stepTitle')}
              value={draft.title}
              onChange={(e) => change(index, { title: e.target.value })}
              placeholder={t('settings.apps.workflowSteps.titlePlaceholder')}
              dataQa="bot-app-workflow-step-title"
            />
            <TextInput
              label={t('settings.apps.workflowSteps.callbackId')}
              value={draft.callback_id}
              onChange={(e) => change(index, { callback_id: e.target.value })}
              placeholder="open_ticket"
              dataQa="bot-app-workflow-step-callback"
            />
            <TextInput
              label={t('settings.apps.workflowSteps.description')}
              optional
              className="sm:col-span-2"
              value={draft.description ?? ''}
              onChange={(e) => change(index, { description: e.target.value })}
              placeholder={t('settings.apps.workflowSteps.descriptionPlaceholder')}
              dataQa="bot-app-workflow-step-description"
            />
          </div>
          <Params
            which="inputs"
            params={draft.inputs ?? []}
            onChange={(inputs) => change(index, { inputs })}
          />
          <Params
            which="outputs"
            params={draft.outputs ?? []}
            onChange={(outputs) => change(index, { outputs })}
          />
          <div className="flex justify-end">
            <IconButton
              label={t('settings.apps.workflowSteps.remove')}
              icon={<Trash2 className="h-4 w-4" />}
              tone="danger"
              onClick={() => setDrafts((current) => current.filter((_, i) => i !== index))}
              data-qa="bot-app-workflow-step-remove"
            />
          </div>
        </div>
      ))}
      <div className="flex flex-wrap items-center gap-2">
        <Button
          type="button"
          size="sm"
          onClick={add}
          disabled={drafts.length >= MAX_STEPS}
          data-qa="bot-app-workflow-step-add"
        >
          <Plus className="h-3.5 w-3.5" /> {t('settings.apps.workflowSteps.add')}
        </Button>
        <Button
          type="submit"
          variant="primary"
          size="sm"
          loading={save.isPending}
          data-qa="bot-app-workflow-steps-save"
        >
          {t('settings.apps.workflowSteps.save')}
        </Button>
        {save.isSuccess && !save.isPending && (
          <span className="text-xs text-muted" data-qa="bot-app-workflow-steps-saved">
            {t('settings.apps.workflowSteps.saved')}
          </span>
        )}
      </div>
      {save.error && (
        <p role="alert" className="text-sm text-danger">
          {toUserMessage(save.error, t('settings.apps.workflowSteps.failed'))}
        </p>
      )}
    </form>
  );
}

interface ParamsProps {
  which: 'inputs' | 'outputs';
  params: Param[];
  onChange: (params: Param[]) => void;
}

function Params({ which, params, onChange }: ParamsProps) {
  const { t } = useTranslation('workspace');
  const set = (index: number, patch: Partial<Param>) =>
    onChange(params.map((param, i) => (i === index ? { ...param, ...patch } : param)));

  return (
    <fieldset className="space-y-2" data-qa={`bot-app-workflow-step-${which}`}>
      <legend className="text-xs font-medium text-fg-dim">{t(`settings.apps.workflowSteps.${which}`)}</legend>
      {params.length === 0 && (
        <p className="text-xs text-muted">
          {t(
            which === 'inputs'
              ? 'settings.apps.workflowSteps.noInputs'
              : 'settings.apps.workflowSteps.noOutputs',
          )}
        </p>
      )}
      {params.map((param, index) => (
        <div
          key={index}
          className="grid grid-cols-1 items-end gap-2 sm:grid-cols-[1fr_1fr_9rem_auto_auto]"
          data-qa="bot-app-workflow-param"
        >
          <TextInput
            label={t('settings.apps.workflowSteps.paramTitle')}
            value={param.title}
            onChange={(e) => set(index, { title: e.target.value })}
            dataQa="bot-app-workflow-param-title"
          />
          <TextInput
            label={t('settings.apps.workflowSteps.paramName')}
            value={param.name}
            placeholder={which === 'inputs' ? 'title' : 'ticket_id'}
            onChange={(e) => set(index, { name: e.target.value })}
            dataQa="bot-app-workflow-param-name"
          />
          <Select
            label={t('settings.apps.workflowSteps.paramKind')}
            value={param.kind}
            onChange={(e) => set(index, { kind: e.target.value as ParamKind })}
            dataQa="bot-app-workflow-param-kind"
          >
            {KINDS.map((kind) => (
              <option key={kind} value={kind}>
                {t(`settings.apps.workflowSteps.kinds.${kind}`)}
              </option>
            ))}
          </Select>
          <Checkbox
            label={t('settings.apps.workflowSteps.required')}
            checked={param.required ?? false}
            onChange={(on) => set(index, { required: on })}
            dataQa="bot-app-workflow-param-required"
          />
          <IconButton
            label={t('settings.apps.workflowSteps.removeParam')}
            icon={<Trash2 className="h-4 w-4" />}
            tone="danger"
            onClick={() => onChange(params.filter((_, i) => i !== index))}
            data-qa="bot-app-workflow-param-remove"
          />
        </div>
      ))}
      <Button
        type="button"
        size="sm"
        variant="ghost"
        disabled={params.length >= MAX_PARAMS}
        onClick={() => onChange([...params, { name: '', title: '', kind: 'text', required: false }])}
        data-qa={`bot-app-workflow-${which}-add`}
      >
        <Plus className="h-3.5 w-3.5" />{' '}
        {t(
          which === 'inputs'
            ? 'settings.apps.workflowSteps.addInput'
            : 'settings.apps.workflowSteps.addOutput',
        )}
      </Button>
    </fieldset>
  );
}
