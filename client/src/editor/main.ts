import { createApp } from 'vue'
import ui from '@nuxt/ui/vue-plugin'
import 'leaflet/dist/leaflet.css'
import './editor.css'
import Editor from './Editor.vue'

createApp(Editor).use(ui).mount('#editor')
