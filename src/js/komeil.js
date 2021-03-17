// Custom scripts
$(document).ready(function () {
    
    
    
    // MetsiMenu Navigation
    $('#side-menu').metisMenu();
    
    // Add class 768 to body when browser window is less than 768 pixels
    $(window).resize(function () {
        if ($(window).width() < 768) {
            $('body').addClass('768'); 
        } else {  
            $('body').removeClass('768');  
        }
    });
    
    // mini navbar
    $('.close-sidebar').click(function () {
        $("body").toggleClass("mini-navbar");
        $('#sidebar-collapse').attr('style', '');
        $("body").removeClass('fixed-sidebar');
        // Toggle right-sidebar if mini-navbar is opened
        if ((!$('body').hasClass('768')) && ($('body').hasClass('right-sidebar')) && (!$('body').hasClass('mini-navbar'))) { $("body").toggleClass("right-sidebar"); }
        if (($('body').hasClass('768')) && ($('body').hasClass('mini-navbar')) && ($('body').hasClass('right-sidebar'))) { $("body").toggleClass("right-sidebar"); }
        if (!$('body').hasClass('mini-navbar')) {
        // Hide menu in order to smoothly turn on when maximize menu
        $('#sidebar-collapse').hide();
        // For smoothly turn on menu
        setTimeout(
            function () {
                $('#sidebar-collapse').fadeIn(500);
            }, 100);
        }
    });
    
    // right sidebar
    $('.reveal-rightsidebar').click(function () {
        $("body").toggleClass("right-sidebar");
        // Toggle mini-navbar if right-sidebar is opened
        if ((!$('body').hasClass('768')) && (!$('body').hasClass('mini-navbar')) && ($('body').hasClass('right-sidebar'))) { $("body").toggleClass("mini-navbar"); }
        if (($('body').hasClass('768')) && ($('body').hasClass('right-sidebar')) && ($('body').hasClass('mini-navbar'))) { $("body").toggleClass("mini-navbar"); }
        if ($('body').hasClass('right-sidebar')) {
        // Hide menu in order to smoothly turn on when maximize menu
        $('#right-sidebar-id').hide();
        // For smoothly turn on menu
        setTimeout(
            function () {
                $('#right-sidebar-id').fadeIn(600);
            }, 100);
        }
    });
    
    // Initialize Tooltips
    $('[data-toggle="tooltip"], .show-tooltip').tooltip({container: 'body', animation: false});
    
    // Initialize Popovers
    $('[data-toggle="popover"]').popover({container: 'body', animation: false});
    
    // Panel Tools
    
    /* Close Panel */
    $(document).ready(function () {
        $(".panel-options .close-panel").click(function() {
            $(this).parents(".panel").fadeToggle(400);
            return false;
        });
    });
    
    /* Minimize */
    $(document).ready(function () {
        $(".panel-options .minimise-panel").click(function (event) {
            $(this).parents(".panel").find(".panel-body").slideToggle(400);
            $(this).parents(".panel").toggleClass('minimized');
            return false;
        });
    }); 
    
    /* expand */
    $(document).ready(function () {
        $('.panel-options .expand-panel').on('click', function () {
            if ($(this).parents(".panel").hasClass('panel-fullsize'))
            {
                $(this).parents(".panel").removeClass('panel-fullsize');
            }
            else
            {
                $(this).parents(".panel").addClass('panel-fullsize');
            }
        });
    });

    /* Fullscreen Toggle - from this resource - http://www.thewebflash.com/2015/04/toggling-fullscreen-mode-using-html5.html */
    function toggleFullscreen(e) {
        e = e || document.documentElement, document.fullscreenElement || document.mozFullScreenElement || document.webkitFullscreenElement || document.msFullscreenElement ? document.exitFullscreen ? document.exitFullscreen() : document.msExitFullscreen ? document.msExitFullscreen() : document.mozCancelFullScreen ? document.mozCancelFullScreen() : document.webkitExitFullscreen && document.webkitExitFullscreen() : e.requestFullscreen ? e.requestFullscreen() : e.msRequestFullscreen ? e.msRequestFullscreen() : e.mozRequestFullScreen ? e.mozRequestFullScreen() : e.webkitRequestFullscreen && e.webkitRequestFullscreen(Element.ALLOW_KEYBOARD_INPUT);
    }
    document.getElementById("btn-fullscreen").addEventListener("click", function () {
        toggleFullscreen();
    });
    
    /* Scroll To Top */
    $(document).ready(function () {
        
        $(window).scroll(function() {
            // If the user scrolls 150 pixels show the scroll to top link
            if ($(this).scrollTop() > 150) {
                $('#to-top').fadeIn(200);
            } else {
                $('#to-top').fadeOut(100);
            }
        });
        
        $('#to-top').click(function() {
            $('html, body').animate({scrollTop: 0}, 400);
            return false;
        });
        
    });  

});