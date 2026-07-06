import type { Meta, StoryObj } from '@storybook/tanstack-react'
import type { SettingsFieldDef } from '@/components/settings/field-defs'
import { FieldRow } from '@/components/settings/field-row'
import { i18nDecorator } from '../../../.storybook/i18n-mock'

const meta: Meta<typeof FieldRow> = {
  title: 'Settings/FieldRow',
  component: FieldRow,
  decorators: [i18nDecorator],
  parameters: {
    layout: 'padded',
    docs: {
      description: {
        component:
          'Single labelled form row. Renders the control matching `field.type`. ' +
          'A modified row gets a primary accent bar on its left edge. ' +
          'Restart-required info is shown only at save time (DiffPreview), not as a per-field badge.',
      },
    },
  },
  args: {
    sectionKey: 'exec',
    onChange: () => {},
    modified: false,
  },
  render: (args) => (
    <div className="mx-auto w-full max-w-2xl divide-y divide-border/40">
      <FieldRow {...args} />
    </div>
  ),
}

export default meta
type Story = StoryObj<typeof FieldRow>

// ── Toggle ─────────────────────────────────────────────────

const toggleField: SettingsFieldDef = {
  key: 'allow_shell_mode',
  labelKey: 'settings.allowShellMode',
  descriptionKey: 'settings.allowShellModeDescription',
  type: 'toggle',
}

export const Toggle: Story = {
  args: { field: toggleField, value: true },
}

// ── Select ─────────────────────────────────────────────────

const selectField: SettingsFieldDef = {
  key: 'default_mode',
  labelKey: 'settings.defaultMode',
  descriptionKey: 'settings.defaultModeDescription',
  type: 'select',
  options: [
    { value: 'structured', labelKey: 'settings.structuredRecommended' },
    { value: 'shell', labelKey: 'settings.shellDangerous' },
  ],
}

export const Select: Story = {
  args: { field: selectField, value: 'structured' },
}

// ── Number ─────────────────────────────────────────────────

const numberField: SettingsFieldDef = {
  key: 'default_timeout_secs',
  labelKey: 'settings.defaultTimeoutS',
  descriptionKey: 'settings.defaultTimeoutSDescription',
  type: 'number',
  placeholder: '120',
  restartScope: 'kernel',
}

export const NumberField: Story = {
  args: { field: numberField, value: 300 },
}

// ── Text (requires a restart) ──────────────────────────────

const textField: SettingsFieldDef = {
  key: 'workspace_path',
  labelKey: 'settings.workspacePath',
  descriptionKey: 'settings.workspacePathDescription',
  type: 'text',
  placeholder: '~/.oxios/workspace',
  restartScope: 'kernel',
}

export const Text: Story = {
  args: { field: textField, value: '~/.oxios/workspace' },
}

// ── Modified accent bar ───────────────────────────────────

export const Modified: Story = {
  args: { field: numberField, value: 900, modified: true },
}

// ── All variants side-by-side ──────────────────────────────

export const AllVariants: Story = {
  render: () => (
    <div className="mx-auto w-full max-w-2xl divide-y divide-border/40">
      <FieldRow sectionKey="exec" field={toggleField} value={false} onChange={() => {}} />
      <FieldRow sectionKey="exec" field={selectField} value="shell" onChange={() => {}} modified />
      <FieldRow sectionKey="exec" field={numberField} value={120} onChange={() => {}} />
      <FieldRow
        sectionKey="exec"
        field={textField}
        value="~/.oxios/workspace"
        onChange={() => {}}
      />
    </div>
  ),
}
