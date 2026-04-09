import { VueRenderer } from '@tiptap/vue-3'
import tippy from 'tippy.js'
import MentionList from './MentionList.vue'
import { invoke } from '@tauri-apps/api/core'

export default {
  items: async ({ query }) => {
    try {
      const res = await invoke('skill_list')
      const skills = res?.skills || res?.data?.skills || []
      
      // Filter only enabled skills and match query
      return skills
        .filter(s => s.enabled)
        .filter(s => s.name.toLowerCase().includes(query.toLowerCase()))
        .map(s => ({
          id: s.id,
          label: s.name,
          description: s.description
        }))
    } catch (err) {
      console.error('Failed to load skills for mention', err)
      return []
    }
  },

  render: () => {
    let component
    let popup

    return {
      onStart: props => {
        component = new VueRenderer(MentionList, {
          props,
          editor: props.editor,
        })

        if (!props.clientRect) {
          return
        }

        popup = tippy('body', {
          getReferenceClientRect: props.clientRect,
          appendTo: () => document.body,
          content: component.element,
          showOnCreate: true,
          interactive: true,
          trigger: 'manual',
          placement: 'top-start',
        })
      },

      onUpdate(props) {
        component.updateProps(props)

        if (!props.clientRect) {
          return
        }

        popup[0].setProps({
          getReferenceClientRect: props.clientRect,
        })
      },

      onKeyDown(props) {
        if (props.event.key === 'Escape') {
          popup[0].hide()
          return true
        }

        return component.ref?.onKeyDown(props)
      },

      onExit() {
        if (popup && popup[0]) {
          popup[0].destroy()
        }
        if (component) {
          component.destroy()
        }
      },
    }
  },
}