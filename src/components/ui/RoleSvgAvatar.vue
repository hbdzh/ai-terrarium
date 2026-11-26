<script setup lang="ts">
import { computed } from "vue";
import { getAvatarSpec } from "../../utils/avatarSvg";

const props = withDefaults(
  defineProps<{
    seed: number;
    size?: number;
  }>(),
  {
    size: 38,
  }
);

const spec = computed(() => getAvatarSpec(props.seed));
</script>

<template>
  <svg
    :width="size"
    :height="size"
    viewBox="0 0 40 40"
    fill="none"
    xmlns="http://www.w3.org/2000/svg"
    class="shrink-0 select-none"
    :style="{ borderRadius: `${Math.round(size * 0.29)}px`, backgroundColor: spec.bg }"
  >
    <!-- Top / Side Details -->
    <template v-if="spec.detail === 'antenna'">
      <line x1="20" y1="6" x2="20" y2="11" :stroke="spec.fg" stroke-width="2" stroke-linecap="round" />
      <circle cx="20" cy="5.5" r="2.2" :fill="spec.accent" :stroke="spec.fg" stroke-width="1.4" />
    </template>
    <template v-else-if="spec.detail === 'halo'">
      <ellipse cx="20" cy="7.5" rx="7.5" ry="2.2" :stroke="spec.accent" stroke-width="2" />
    </template>
    <template v-else-if="spec.detail === 'ears'">
      <circle cx="9.5" cy="20" r="3" :fill="spec.accent" />
      <circle cx="30.5" cy="20" r="3" :fill="spec.accent" />
    </template>

    <!-- Head Shape -->
    <rect
      v-if="spec.headShape === 'rounded'"
      x="10.5"
      y="11"
      width="19"
      height="18"
      rx="6"
      :fill="'#FFFFFF'"
      :stroke="spec.fg"
      stroke-width="2"
    />
    <circle
      v-else-if="spec.headShape === 'circle'"
      cx="20"
      cy="20.5"
      r="9.5"
      :fill="'#FFFFFF'"
      :stroke="spec.fg"
      stroke-width="2"
    />
    <polygon
      v-else-if="spec.headShape === 'hex'"
      points="20,10.5 29,15.5 29,25.5 20,30.5 11,25.5 11,15.5"
      :fill="'#FFFFFF'"
      :stroke="spec.fg"
      stroke-width="2"
      stroke-linejoin="round"
    />
    <rect
      v-else
      x="11"
      y="11.5"
      width="18"
      height="18"
      rx="4"
      :fill="'#FFFFFF'"
      :stroke="spec.fg"
      stroke-width="2"
    />

    <!-- Eyes -->
    <template v-if="spec.eyes === 'dots'">
      <circle cx="16.5" cy="19.5" r="1.7" :fill="spec.fg" />
      <circle cx="23.5" cy="19.5" r="1.7" :fill="spec.fg" />
      <path d="M17.2 24C18.2 25.1 21.8 25.1 22.8 24" :stroke="spec.fg" stroke-width="1.6" stroke-linecap="round" />
    </template>
    <template v-else-if="spec.eyes === 'visor'">
      <rect x="13.5" y="16.8" width="13" height="5" rx="2.5" :fill="spec.accent" :stroke="spec.fg" stroke-width="1.5" />
      <line x1="17.5" y1="24.5" x2="22.5" y2="24.5" :stroke="spec.fg" stroke-width="1.6" stroke-linecap="round" />
    </template>
    <template v-else-if="spec.eyes === 'happy'">
      <path d="M14.8 19.5C15.5 18.2 17.5 18.2 18.2 19.5" :stroke="spec.fg" stroke-width="1.8" stroke-linecap="round" />
      <path d="M21.8 19.5C22.5 18.2 24.5 18.2 25.2 19.5" :stroke="spec.fg" stroke-width="1.8" stroke-linecap="round" />
      <path d="M17.2 23.8C18.3 25.2 21.7 25.2 22.8 23.8" :stroke="spec.fg" stroke-width="1.6" stroke-linecap="round" />
    </template>
    <template v-else>
      <circle cx="16.5" cy="19.5" r="1.7" :fill="spec.fg" />
      <path d="M21.8 19.5H25" :stroke="spec.fg" stroke-width="1.8" stroke-linecap="round" />
      <path d="M17.5 24C18.6 25 21.4 25 22.5 24" :stroke="spec.fg" stroke-width="1.6" stroke-linecap="round" />
    </template>
  </svg>
</template>
