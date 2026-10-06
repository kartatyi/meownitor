// The widget's words in English (the default) and Ukrainian. The choice lives in config.json
// (`lang`); Rust reads it too, for the tray menu and the round windows' titles (i18n.rs).
(function(){
  var S={
    en:{
      'kind.cat':'Cat','kind.blob':'Blob','kind.ghost':'Ghost',
      'u.s':'{n} s','u.min':'{n} min','u.h':'{n} h','u.d':'{n} d','yesterday':'yesterday','ago':'{t} ago',
      'lim.session':'5 hours','lim.week':'Week',
      'lim.none':'Limits: {why}','lim.net':'no connection','lim.busy':'the server asks to wait, will try later',
      'lim.login':'sign in to Claude Code — <code>claude</code> in a terminal','lim.stale':'waiting for Claude Code to renew its sign-in','lim.updated':'Updated {ago}',
      'what.think':'Thinking','what.ask':'Asks you','what.agent':'Agent','what.perm':'Waiting for permission',
      'what.done':'Done — your turn','what.failed':'Stopped with an error',
      'answer':'Answer','open.desktop':'Open in Desktop',
      'active':['{n} active'],'quiet':'all quiet','settings':'Settings','minimize':'Minimize',
      'g.wait':'Waiting for you','g.run':'Working','g.done':'Your turn','g.idle':'Idle',
      'empty':'No sessions in the last day','terminal':'{name} · terminal','session':'Session {id}',
      'hook.ask.install':'Add the widget’s hook to 11 events in ~/.claude/settings.json? Other hooks stay untouched; a copy of the file is kept next to it.',
      'hook.ask.uninstall':'Remove the widget’s hook from ~/.claude/settings.json? Sessions will stop updating. Other hooks stay untouched; a copy of the file is kept next to it.',
      'hook.add':'Add','hook.remove':'Remove','no':'No',
      'hook':'Claude Code hook','hook.on':'installed — sessions are visible','hook.off':'not installed — no sessions',
      'hook.uninstall':'Remove','hook.install':'Install',
      'back':'Back to sessions','character':'Character','sound':'Sound when a session asks','sound.t':'Sound',
      'autostart':'Start with Windows','autostart.mac':'Open at login','autostart.t':'Autostart','autostart.err':'Autostart: {e}',
      'language':'Language','check':'check'
    },
    uk:{
      'kind.cat':'Котик','kind.blob':'Краплинка','kind.ghost':'Привидок',
      'u.s':'{n} с','u.min':'{n} хв','u.h':'{n} г','u.d':'{n} д','yesterday':'вчора','ago':'{t} тому',
      'lim.session':'5 годин','lim.week':'Тиждень',
      'lim.none':'Ліміти: {why}','lim.net':'нема з’єднання','lim.busy':'сервер просить зачекати, спробую пізніше',
      'lim.login':'увійди в Claude Code — <code>claude</code> у терміналі','lim.stale':'чекаю, поки Claude Code оновить вхід','lim.updated':'Оновлено {ago}',
      'what.think':'Думає','what.ask':'Питає тебе','what.agent':'Агент','what.perm':'Чекає дозволу',
      'what.done':'Готово — твоя черга','what.failed':'Зупинилась з помилкою',
      'answer':'Відповісти','open.desktop':'Відкрити в Desktop',
      'active':['{n} активна','{n} активні','{n} активних'],'quiet':'усе тихо','settings':'Налаштування','minimize':'Згорнути',
      'g.wait':'Чекають на тебе','g.run':'Працюють','g.done':'Твоя черга','g.idle':'Неактивні',
      'empty':'Сесій за добу нема','terminal':'{name} · термінал','session':'Сесія {id}',
      'hook.ask.install':'Додати хук віджета на 11 подій у ~/.claude/settings.json? Інші хуки не зачіпаю, копія файлу буде поруч.',
      'hook.ask.uninstall':'Прибрати хук віджета з ~/.claude/settings.json? Сесії перестануть оновлюватись. Інші хуки не зачіпаю, копія файлу буде поруч.',
      'hook.add':'Додати','hook.remove':'Прибрати','no':'Ні',
      'hook':'Хук Claude Code','hook.on':'стоїть — сесії видно','hook.off':'не стоїть — сесій не видно',
      'hook.uninstall':'Зняти','hook.install':'Поставити',
      'back':'Назад до сесій','character':'Персонаж','sound':'Звук, коли сесія питає','sound.t':'Звук',
      'autostart':'Запускати з Windows','autostart.mac':'Запускати під час входу','autostart.t':'Автозапуск','autostart.err':'Автозапуск: {e}',
      'language':'Мова','check':'перевірка'
    }
  };
  var lang='en';
  // Plural forms by count: English one/other, Ukrainian one/few/many.
  function form(forms,n){
    if(forms.length<3)return forms[n===1||forms.length<2?0:1];
    var d=n%10,h=n%100;return forms[d===1&&h!==11?0:d>=2&&d<=4&&(h<12||h>14)?1:2];
  }
  // A key's text in the current language (English when it has none), with {name} filled from `a`;
  // a key with plural forms picks one by a.n.
  function t(k,a){
    var s=S[lang][k];if(s==null)s=S.en[k];if(s==null)return k;
    if(Array.isArray(s))s=form(s,a&&a.n||0);
    return a?s.replace(/\{(\w+)\}/g,function(m,n){return a[n]!=null?a[n]:m;}):s;
  }
  function set(l){lang=S[l]?l:'en';document.documentElement.lang=lang;}
  window.I18N={t:t,set:set,lang:function(){return lang;},LANGS:[['en','English'],['uk','Українська']]};
})();
