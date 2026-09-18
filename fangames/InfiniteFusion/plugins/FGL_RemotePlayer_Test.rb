# FGL_RemotePlayer_Test.rb — prototype Game_Character + Sprite_Character + IPC
# Source unique: fangames/InfiniteFusion/plugins/
# map_id distant n impose PAS la map locale

module FGL_RemotePlayer_Test
  TICK = 0.05
  STALE_MISS = 120
  STATUS_EVERY = 30

  @sock = nil
  @my_id = nil
  @last = 0.0
  @remotes = {}
  @hooks_done = false
  @status_n = 0
  @last_err = ""

  def self.status_path
    begin
      desk = ENV["USERPROFILE"].to_s
      return File.join(desk, "Desktop", "fgl_rpt_status.txt") if desk != ""
    rescue
    end
    begin
      return File.join(ENV["TEMP"].to_s, "fgl_rpt_status.txt")
    rescue
    end
    "fgl_rpt_status.txt"
  end

  def self.write_status(extra = "")
    begin
      lines = []
      lines << "time=#{Time.now}"
      lines << "ipc_port=#{ipc_port}"
      lines << "my_id=#{@my_id}"
      lines << "poll_raw=#{@poll_raw.to_i}"
      lines << "remotes=#{@remotes.size}"
      lines << "scene=#{$scene ? $scene.class.name : "nil"}"
      lines << "vp=#{map_viewport ? "ok" : "nil"}"
      lines << "err=#{@last_err}"
      @remotes.each do |id, rec|
        d = rec[:data]
        lines << "remote id=#{id} map=#{d[:map]} x=#{d[:x]} y=#{d[:y]} dir=#{d[:dir]} spr=#{rec[:sprite] ? "yes" : "no"}"
      end
      lines << extra if extra.to_s != ""
      File.open(status_path, "w") { |f| f.puts lines.join("\n") }
    rescue
    end
  end

  def self.ipc_port
    p = ENV["FGL_IPC_PORT"].to_s.to_i
    return 0 if p < 1 || p > 65535
    p
  end

  def self.ensure_sock
    return if @sock
    begin
      require "socket"
      @sock = UDPSocket.new
      @sock.bind("127.0.0.1", 0)
    rescue => e
      @last_err = "sock:#{e}"
      @sock = nil
    end
  end

  def self.ipc_send(line)
    p = ipc_port
    return if p <= 0
    ensure_sock
    return unless @sock
    begin
      @sock.send("PLAYER|" + line.to_s, 0, "127.0.0.1", p)
    rescue => e
      @last_err = "send:#{e}"
    end
  end

  def self.ipc_poll
    ensure_sock
    return [] unless @sock
    out = []
    32.times do
      begin
        ready = false
        begin
          if defined?(IO) && IO.respond_to?(:select)
            r = IO.select([@sock], nil, nil, 0)
            ready = r && r[0] && r[0].include?(@sock)
          else
            ready = true
          end
        rescue
          ready = true
        end
        break unless ready
        data, _addr = @sock.recvfrom(65535)
        out << data.to_s if data && data.to_s.length > 0
      rescue Errno::EAGAIN, Errno::EWOULDBLOCK
        break
      rescue
        break
      end
    end
    out
  end

  def self.ensure_id
    return if @my_id
    @my_id = "#{Time.now.to_i}_#{rand(999999)}"
  end

  def self.safe_text(s)
    s.to_s.gsub("|", "").gsub("\n", " ").strip[0, 32]
  end

  def self.clean_name(path)
    return "walk" if path.nil? || path.to_s.empty?
    s = path.to_s.gsub("\\", "/")
    s = s.sub(%r{^Graphics/Characters/}i, "")
    s = s.sub(%r{^/+}, "")
    s = s.sub(/\.(png|jpg|bmp)$/i, "")
    s = s.gsub("|", "")
    s.empty? ? "walk" : s
  end

  def self.map_viewport
    begin
      return Spriteset_Map.viewport if defined?(Spriteset_Map) && Spriteset_Map.respond_to?(:viewport)
    rescue
    end
    return nil unless $scene
    s = nil
    begin
      s = $scene.spriteset if $scene.respond_to?(:spriteset)
    rescue
    end
    if s.nil?
      begin
        s = $scene.instance_variable_get(:@spriteset)
      rescue
      end
    end
    return nil unless s
    begin
      return s.viewport1 if s.respond_to?(:viewport1)
    rescue
    end
    begin
      v = s.instance_variable_get(:@viewport1)
      return v if v
    rescue
    end
    begin
      v = s.instance_variable_get(:@viewport)
      return v if v
    rescue
    end
    nil
  end

  def self.write_local
    return unless $game_player && $game_map
    ensure_id
    p = $game_player
    cn = "walk"
    begin
      cn = p.character_name.to_s
    rescue
    end
    cname = clean_name(cn)
    px = 0
    py = 0
    pdir = 2
    pspeed = 3
    ppat = 0
    mid = 0
    begin; px = p.x.to_i; rescue; end
    begin; py = p.y.to_i; rescue; end
    begin; pdir = p.direction.to_i; rescue; end
    begin; pspeed = p.move_speed.to_i; rescue; end
    begin; ppat = p.pattern.to_i; rescue; end
    begin; mid = $game_map.map_id.to_i; rescue; end
    pname = "Player"
    begin
      pname = safe_text($player.name) if defined?($player) && $player && $player.name
    rescue
    end
    begin
      pname = safe_text($Trainer.name) if pname == "Player" && defined?($Trainer) && $Trainer && $Trainer.name
    rescue
    end
    line = [
      @my_id, "P", mid, px, py, pdir, cname, pspeed, ppat,
      0, 0, 0, 0, 0, 0, "", pname
    ].join("|")
    ipc_send(line)
  end

  def self.parse_player_raw(raw)
    s = raw.to_s
    s = s[7, s.length - 7] if s.index("PLAYER|") == 0
    a = s.strip.split("|")
    return nil if a.size < 7
    id = a[0].to_s
    return nil if id.empty?
    return nil if @my_id && id == @my_id
    return nil if a[1].to_s != "P" && a[1].to_s != ""
    {
      :id => id,
      :map => a[2].to_i,
      :x => a[3].to_i,
      :y => a[4].to_i,
      :dir => a[5].to_i,
      :cname => clean_name(a[6]),
      :speed => a[7].to_i,
      :pattern => a[8].to_i,
      :pname => (a.size > 16 ? safe_text(a[16]) : "Player")
    }
  end

  def self.ensure_sprite(rec)
    return if rec[:sprite] && !(rec[:sprite].disposed? rescue true)
    v = map_viewport
    return unless v
    return unless defined?(Sprite_Character)
    begin
      rec[:sprite] = Sprite_Character.new(v, rec[:char])
      begin
        rec[:sprite].visible = true
      rescue
      end
    rescue => e
      @last_err = "sprite:#{e}"
      rec[:sprite] = nil
    end
  end

  def self.ingest_network
    ensure_id
    seen = {}
    batch = ipc_poll
    @poll_raw = (@poll_raw || 0) + batch.size
    batch.each do |raw|
      data = parse_player_raw(raw)
      next unless data
      id = data[:id]
      seen[id] = true
      rec = @remotes[id]
      if rec.nil?
        char = FGL_RemoteCharacter.new
        char.apply_net(data)
        rec = { :char => char, :sprite => nil, :miss => 0, :data => data }
        @remotes[id] = rec
        ensure_sprite(rec)
      else
        rec[:char].apply_net(data)
        rec[:data] = data
        rec[:miss] = 0
        ensure_sprite(rec)
      end
    end
    @remotes.keys.each do |id|
      next if seen[id]
      @remotes[id][:miss] = (@remotes[id][:miss] || 0) + 1
      kill_remote(id) if @remotes[id][:miss] > STALE_MISS
    end
  end

  def self.kill_remote(id)
    rec = @remotes[id]
    return unless rec
    begin
      if rec[:sprite]
        rec[:sprite].dispose unless (rec[:sprite].disposed? rescue false)
      end
    rescue
    end
    @remotes.delete(id)
  end

  def self.update_sprites
    @remotes.each_value do |rec|
      begin
        rec[:char].update if rec[:char].respond_to?(:update)
      rescue
      end
      ensure_sprite(rec)
      begin
        if rec[:sprite] && !(rec[:sprite].disposed? rescue true)
          rec[:sprite].update
          rec[:sprite].visible = true
        end
      rescue => e
        @last_err = "upd:#{e}"
      end
    end
  end

  def self.clear_all
    @remotes.keys.each { |id| kill_remote(id) }
    @remotes.clear
  end

  def self.tick
    return unless $game_player && $game_map
    now = Time.now.to_f
    begin
      now = System.uptime
    rescue
    end
    return if now - @last < TICK
    @last = now
    begin
      write_local
    rescue => e
      @last_err = "write:#{e}"
    end
    begin
      return unless $scene.is_a?(Scene_Map)
    rescue
      return
    end
    begin
      ingest_network
    rescue => e
      @last_err = "ingest:#{e}"
    end
    begin
      update_sprites
    rescue => e
      @last_err = "sprites:#{e}"
    end
    @status_n += 1
    if @status_n == 1 || (@status_n % STATUS_EVERY) == 0
      write_status("tick=#{@status_n}")
    end
  end

  def self.install_hooks!
    return if @hooks_done
    ok = false
    begin
      if defined?(Graphics)
        meta = (class << Graphics; self; end)
        meta.class_eval do
          unless method_defined?(:_fgl_rpt_graphics_update)
            alias_method :_fgl_rpt_graphics_update, :update
            def update
              _fgl_rpt_graphics_update
              begin
                FGL_RemotePlayer_Test.tick
              rescue
              end
            end
          end
        end
        ok = true
      end
    rescue => e
      @last_err = "hookG:#{e}"
    end
    begin
      if defined?(Scene_Map) && Scene_Map.method_defined?(:update)
        Scene_Map.class_eval do
          unless method_defined?(:_fgl_rpt_scene_update)
            alias_method :_fgl_rpt_scene_update, :update
            def update
              _fgl_rpt_scene_update
              begin
                FGL_RemotePlayer_Test.update_sprites
              rescue
              end
            end
          end
        end
        ok = true
      end
    rescue => e
      @last_err = "hookS:#{e}"
    end
    begin
      if defined?(EventHandlers)
        EventHandlers.add(:on_new_spriteset_map, :fgl_remote_player_test_clear, proc {
          begin
            FGL_RemotePlayer_Test.clear_all
          rescue
          end
        })
      end
    rescue
    end
    @hooks_done = ok
    write_status("hooks=#{ok}")
  end
end

class FGL_RemoteCharacter < Game_Character
  attr_reader :net_id, :net_map_id, :net_pname

  def initialize
    begin
      super()
    rescue ArgumentError
      begin
        super($game_map)
      rescue
        @x = 0
        @y = 0
        @real_x = 0
        @real_y = 0
        @direction = 2
        @pattern = 0
        @move_speed = 3
        @character_name = "walk"
        @character_hue = 0
        @opacity = 255
        @blend_type = 0
        @tile_id = 0
      end
    end
    @net_id = nil
    @net_map_id = 0
    @net_pname = "Player"
    @through = true
    @move_frequency = 6
    @walk_anime = true
    @step_anime = false
    @direction_fix = false
    @opacity = 255
  end

  def apply_net(data)
    @net_id = data[:id]
    @net_map_id = data[:map].to_i
    @net_pname = data[:pname].to_s
    @net_pname = "Player" if @net_pname.empty?
    nx = data[:x].to_i
    ny = data[:y].to_i
    ndir = data[:dir].to_i
    ndir = 2 if ndir <= 0
    npat = data[:pattern].to_i
    nspd = data[:speed].to_i
    nspd = 3 if nspd <= 0
    ncname = data[:cname].to_s
    ncname = "walk" if ncname.empty?
    begin
      if respond_to?(:moveto)
        moveto(nx, ny)
      else
        @x = nx
        @y = ny
        if defined?(Game_Map::REAL_RES_X)
          @real_x = nx * Game_Map::REAL_RES_X
          @real_y = ny * Game_Map::REAL_RES_Y
        else
          @real_x = nx * 128
          @real_y = ny * 128
        end
      end
    rescue
      @x = nx
      @y = ny
    end
    @direction = ndir
    @pattern = npat
    @move_speed = nspd
    begin
      @character_name = ncname
    rescue
      instance_variable_set(:@character_name, ncname)
    end
    @opacity = 255
    @through = true
    @walk_anime = true
  end
end

begin
  FGL_RemotePlayer_Test.install_hooks!
rescue
end