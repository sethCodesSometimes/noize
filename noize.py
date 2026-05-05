#!/usr/bin/env python3
import mpv
import sys
import os
import json

class AudioMixer:
    def __init__(self, config_path="config.json", fresh=False):
        self.sources = []
        self.current_index = 0
        self.max_volume = 10
        self.volume_step = 1
        self._dirty = True
        self.state_path = "state.json"
        self._global_mute = False
        self._saved_states = None
        self.load_config(config_path)
        if not fresh:
            self.load_state()
        
    def load_config(self, config_path):
        with open(config_path) as f:
            config = json.load(f)
        self.max_volume = config.get("max_volume", 10)
        self.volume_step = config.get("volume_step", 1)
        self.config_sources = []
        for src in config.get("sources", []):
            urls = src["url"] if isinstance(src["url"], list) else [src["url"]]
            name = src.get("name", "")
            self.config_sources.append({"urls": urls, "name": name})
            self.add_source(urls, name)
            
    def load_state(self):
        if not os.path.isfile(self.state_path):
            return
        try:
            with open(self.state_path) as f:
                state = json.load(f)
            for i, src in enumerate(self.sources):
                if i < len(state.get("sources", [])):
                    saved = state["sources"][i]
                    src['volume'] = saved.get('volume', 0)
                    src['muted'] = saved.get('muted', False)
                    src['previous_volume'] = saved.get('previous_volume', 0)
                    src['player'].volume = 0 if src['muted'] else self.perceived_volume(src['volume'])
            self.current_index = state.get('current_index', 0)
            self._dirty = True
        except (json.JSONDecodeError, IOError):
            pass
            
    def save_state(self):
        state = {
            "current_index": self.current_index,
            "sources": []
        }
        for src in self.sources:
            state["sources"].append({
                "url": src['urls'][0],  # Just save first URL for matching
                "volume": src['volume'],
                "muted": src.get('muted', False),
                "previous_volume": src.get('previous_volume', 0)
            })
        with open(self.state_path, 'w') as f:
            json.dump(state, f, indent=2)
        
    def add_source(self, urls, name=None):
        if isinstance(urls, str):
            urls = [urls]
        is_local = all(os.path.isfile(u) for u in urls)
        player = mpv.MPV(ytdl=not is_local, video=False)
        
        source_data = {
            'player': player,
            'name': name or (urls[0][:40] if urls else "Unknown"),
            'volume': 0,
            'urls': urls,
            'url_index': 0,
            'muted': False,
            'previous_volume': 0
        }
        
        self.sources.append(source_data)
        player.play(urls[0])
        player.volume = 0
        
        def try_next_url():
            if source_data['url_index'] + 1 < len(urls):
                source_data['url_index'] += 1
                player.play(urls[source_data['url_index']])
                return True
            return False
        
        @player.event_callback('end-file')
        def on_end_file(event):
            if hasattr(event, 'reason') and event.reason in [2, 3, 4]:
                try_next_url()
        
    def adjust_volume(self, delta):
        if not self.sources:
            return
        source = self.sources[self.current_index]
        new_vol = max(0, min(self.max_volume, source['volume'] + delta))
        source['volume'] = new_vol
        perceived = self.perceived_volume(new_vol)
        source['player'].volume = perceived
        self._dirty = True
        
    def perceived_volume(self, step):
        return 100 * (step / self.max_volume)
        
    def toggle_mute(self):
        if not self.sources:
            return
        source = self.sources[self.current_index]
        if source.get('muted'):
            source['volume'] = source.get('previous_volume', 0)
            source['player'].volume = self.perceived_volume(source['volume'])
            source['muted'] = False
        else:
            source['previous_volume'] = source['volume']
            source['volume'] = 0
            source['player'].volume = 0
            source['muted'] = True
        self._dirty = True

    def toggle_global_mute(self):
        if not self.sources:
            return
        if self._global_mute:
            for i, src in enumerate(self.sources):
                saved = self._saved_states[i]
                src['volume'] = saved['volume']
                src['muted'] = saved['muted']
                src['previous_volume'] = saved['previous_volume']
                src['player'].volume = 0 if src['muted'] else self.perceived_volume(src['volume'])
            self._global_mute = False
            self._saved_states = None
        else:
            self._saved_states = []
            for src in self.sources:
                state = {
                    'volume': src['volume'],
                    'muted': src['muted'],
                    'previous_volume': src.get('previous_volume', 0)
                }
                self._saved_states.append(state)
                src['previous_volume'] = src['volume']
                src['volume'] = 0
                src['muted'] = True
                src['player'].volume = 0
            self._global_mute = True
        self._dirty = True

    def scroll_up(self):
        if self.sources:
            self.current_index = (self.current_index - 1) % len(self.sources)
            self._dirty = True
            
    def scroll_down(self):
        if self.sources:
            self.current_index = (self.current_index + 1) % len(self.sources)
            self._dirty = True
            
    def set_volume_direct(self, level):
        if not self.sources:
            return
        source = self.sources[self.current_index]
        source['muted'] = False
        source['volume'] = level
        source['player'].volume = self.perceived_volume(level)
        self._dirty = True
        
    def display(self, force=False):
        if not force and not self._dirty:
            return
        self._dirty = False
        
        output = "\033[2J\033[H" if force else "\033[H"  # Clear screen if forced
        output += "=== noize ===\n"
        for i, src in enumerate(self.sources):
            marker = ">" if i == self.current_index else " "
            vol = src['previous_volume'] if src.get('muted') else src['volume']
            bar = "#" * vol + "-" * (self.max_volume - vol)
            mute_indicator = " [MUTED]" if src.get('muted') else ""
            url_info = f" [{src['url_index']+1}/{len(src['urls'])}]" if len(src['urls']) > 1 else ""
            output += f"{marker} {src['name'][:30]:30} [{bar}] {vol}/{self.max_volume}{mute_indicator}{url_info}\n"
        output += "\nControls: j/k=scroll, h/l=volume down/up, m=mute/unmute, space=global mute, 0-9=set volume, q=quit\n"
        if self._global_mute:
            output += "*** GLOBAL MUTE ACTIVE ***\n"
        output += "\033[J"  # Clear from cursor to end
        sys.stdout.write(output)
        sys.stdout.flush()
        
    def run(self):
        import select
        import time
        buffer = ""
        last_digit_time = 0
        timeout = 0.3
        self.display(force=True)
        while True:
            if buffer and (time.time() - last_digit_time > timeout):
                num = int(buffer)
                if num <= self.max_volume:
                    self.set_volume_direct(num)
                buffer = ""
            
            if self._dirty:
                self.display()
            
            try:
                ready = select.select([sys.stdin], [], [], 0.1)[0]
                if ready:
                    key = sys.stdin.read(1).lower()
                    if key == 'q':
                        break
                    elif key == 'j':
                        buffer = ""
                        self.scroll_down()
                    elif key == 'k':
                        buffer = ""
                        self.scroll_up()
                    elif key == 'h':
                        buffer = ""
                        self.adjust_volume(-self.volume_step)
                    elif key == 'l':
                        buffer = ""
                        self.adjust_volume(self.volume_step)
                    elif key == 'm':
                        buffer = ""
                        self.toggle_mute()
                    elif key == ' ':
                        buffer = ""
                        self.toggle_global_mute()
                    elif key.isdigit():
                        if key == '0' and len(buffer) == 0:
                            self.set_volume_direct(0)
                            buffer = ""
                        else:
                            buffer += key
                            last_digit_time = time.time()
                            if buffer == '10':
                                self.set_volume_direct(10)
                                buffer = ""
                            elif len(buffer) >= 2:
                                num = int(buffer)
                                if num <= self.max_volume:
                                    self.set_volume_direct(num)
                                buffer = ""
            except KeyboardInterrupt:
                break
                
        self.save_state()
        for src in self.sources:
            src['player'].terminate()

if __name__ == "__main__":
    import tty
    import termios
    import argparse
    
    parser = argparse.ArgumentParser()
    parser.add_argument("--config", default="config.json", help="Path to config file")
    parser.add_argument("--fresh", action="store_true", help="Start with zero volumes (don't load saved state)")
    args = parser.parse_args()
    
    mixer = AudioMixer(args.config, fresh=args.fresh)
    
    old_settings = termios.tcgetattr(sys.stdin)
    try:
        tty.setcbreak(sys.stdin.fileno())
        mixer.run()
    finally:
        termios.tcsetattr(sys.stdin, termios.TCSADRAIN, old_settings)
