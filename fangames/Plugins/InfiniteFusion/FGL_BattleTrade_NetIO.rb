#===============================================================================
# Remplace DIR/FGL_peers : toutes lectures/ecritures Battle & Trade -> FGLIPCBus
# Aucun fichier disque. Actions combat incluses (MOVE/SWITCH/RUN/party bins).
#===============================================================================

if defined?(FGLBattle)
  module FGLBattle
    def self.ensure_dir
      # no-op : pas de dossier
    end

    def self.write_file(name, text)
      FGLIPCBus.put(name.to_s, text.to_s, false)
    end

    def self.write_bin(name, data)
      # data deja binary string (Marshal.dump fait coté caller parfois)
      if data.is_a?(String)
        FGLIPCBus.put(name.to_s, data, true)
      else
        FGLIPCBus.put_bin(name.to_s, data)
      end
    end

    def self.read_file(name)
      FGLIPCBus.poll
      FGLIPCBus.get(name.to_s)
    end

    def self.delete_file(name)
      FGLIPCBus.del(name.to_s)
    end

    def self.my_id
      begin
        if defined?(FGL_RemotePlayer_Test)
          id = FGL_RemotePlayer_Test.instance_variable_get(:@my_id) rescue nil
          return id if id && id.to_s != ""
        end
      rescue; end
      begin
        return FGL.instance_variable_get(:@my_id) if defined?(FGL)
      rescue; end
      nil
    end

    # peer_is_busy: plus de Dir.glob disque
    def self.peer_is_busy?(peer_id)
      return true if peer_id.nil?
      FGLIPCBus.poll
      FGLIPCBus.glob("chal_*").each do |k|
        raw = FGLIPCBus.get(k).to_s
        return true if raw.include?(peer_id.to_s) || k.include?(peer_id.to_s)
      end
      false
    end

    def self.find_peer_by_name(name)
      name_l = name.to_s.downcase.strip
      begin
        if defined?(FGL_RemotePlayer_Test)
          rem = FGL_RemotePlayer_Test.instance_variable_get(:@remotes) rescue nil
          if rem.is_a?(Hash)
            rem.each do |id, rec|
              pname = (rec[:pname] || rec[:name] || "").to_s
              next if pname.downcase.strip != name_l
              return [id, pname, {}]
            end
          end
        end
      rescue; end
      nil
    end
  end
end

if defined?(FGLTrade)
  module FGLTrade
    def self.ensure_dir
    end

    def self.write_text(name, text)
      FGLIPCBus.put(name.to_s, text.to_s, false)
    end

    def self.read_text(name)
      FGLIPCBus.poll
      FGLIPCBus.get(name.to_s)
    end

    def self.write_bin(name, obj)
      FGLIPCBus.put_bin(name.to_s, obj)
    end

    def self.read_bin(name)
      FGLIPCBus.poll
      FGLIPCBus.get_bin(name.to_s)
    end

    def self.delete_file(name)
      FGLIPCBus.del(name.to_s)
    end

    def self.my_id
      begin
        if defined?(FGL_RemotePlayer_Test)
          id = FGL_RemotePlayer_Test.instance_variable_get(:@my_id) rescue nil
          return id if id && id.to_s != ""
        end
      rescue; end
      FGL.instance_variable_get(:@my_id) rescue nil
    end

    def self.peer_is_busy?(peer_id)
      return true if peer_id.nil?
      FGLIPCBus.poll
      FGLIPCBus.glob("trd_*").each do |k|
        raw = FGLIPCBus.get(k).to_s
        return true if raw.include?(peer_id.to_s) || k.include?(peer_id.to_s)
      end
      false
    end

    def self.find_peer_by_name(name)
      name_l = name.to_s.downcase.strip
      begin
        if defined?(FGL_RemotePlayer_Test)
          rem = FGL_RemotePlayer_Test.instance_variable_get(:@remotes) rescue nil
          if rem.is_a?(Hash)
            rem.each do |id, rec|
              pname = (rec[:pname] || rec[:name] || "").to_s
              return [id, pname] if pname.downcase.strip == name_l
            end
          end
        end
      rescue; end
      nil
    end

    def self.all_connected_peers
      list = []
      mid = my_id.to_s
      begin
        if defined?(FGL_RemotePlayer_Test)
          rem = FGL_RemotePlayer_Test.instance_variable_get(:@remotes) rescue nil
          if rem.is_a?(Hash)
            rem.each do |id, rec|
              next if id.to_s == mid
              n = (rec[:pname] || rec[:name] || id).to_s
              list << [id, n]
            end
          end
        end
      rescue; end
      list
    end

    # tick : remplacer Dir.glob fichier par glob memoire reseau
    def self.tick
      now = Time.now.to_f rescue 0.0
      return if now - @last < TICK
      @last = now
      mid = my_id
      return if mid.nil?
      FGLIPCBus.poll
      begin; poll_commands; rescue; end
      if @busy && !@active && @trade_id
        ans = read_text("trd_ans_#{@trade_id}.txt")
        if ans
          if ans.include?("ACCEPT")
            @active = true
            start_trade_flow(true)
          elsif ans.include?("REFUSE")
            force_msg_bottom_system!
            pbMessage("\\wd" + _INTL("Trade request declined.")) rescue nil
            cleanup
          end
        end
        return
      end
      return if busy?
      battle_busy = false
      begin
        battle_busy = true if defined?(FGLBattle) && (FGLBattle.busy? || FGLBattle.active?)
      rescue; end
      return if battle_busy
      FGLIPCBus.glob("trd_req_*.txt").each do |k|
        begin
          raw = FGLIPCBus.get(k).to_s
          parts = raw.strip.split("|")
          next if parts.length < 4
          from_id = parts[0]
          to_id = parts[1]
          from_name = parts[2]
          kind = parts[3]
          next unless to_id.to_s == mid.to_s
          next unless kind == "REQ"
          tid = k.sub("trd_req_", "").sub(".txt", "")
          next if read_text("trd_ans_#{tid}.txt")
          accept_request(tid, from_id, from_name)
          break
        rescue; end
      end
    end
  end
end